use rand::Rng;
use resvg::usvg::{Options, Tree};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

pub const CANVAS_SIZE: u32 = 512;
pub const HOTSPOT_X: i32 = 180;
pub const HOTSPOT_Y: i32 = 150;

const SVG_TIP_X: f32 = 90.0;
const SVG_TIP_Y: f32 = 22.0;
const BASE_ANGLE_DEG: f32 = -22.0;
const BASE_HAND_SCALE: f32 = 0.45;

const HAND_SVG: &str = r##"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 320" width="240" height="320">
  <defs>
    <linearGradient id="armFade" x1="0" y1="185" x2="0" y2="295" gradientUnits="userSpaceOnUse">
      <stop offset="0%" stop-color="#000000" stop-opacity="1.0" />
      <stop offset="55%" stop-color="#000000" stop-opacity="0.85" />
      <stop offset="100%" stop-color="#000000" stop-opacity="0.0" />
    </linearGradient>
  </defs>

  <path d="M 88 182 L 146 182 L 154 295 L 80 295 Z" fill="url(#armFade)" />

  <g transform="translate(4, 6)" opacity="0.22">
    <path d="M 76 108
             L 76 34 C 76 14, 104 14, 104 34 L 104 88
             C 106 72, 128 72, 130 88
             C 134 76, 154 78, 154 94
             C 158 84, 176 90, 174 108
             C 174 136, 160 156, 146 164
             L 86 164
             C 72 154, 62 140, 46 124
             C 30 108, 48 86, 66 102 Z"
          fill="#000000" />
    <rect x="76" y="158" width="80" height="30" rx="10" fill="#000000" />
  </g>

  <path d="M 76 108
           L 76 34 C 76 14, 104 14, 104 34 L 104 88
           C 106 72, 128 72, 130 88
           C 134 76, 154 78, 154 94
           C 158 84, 176 90, 174 108
           C 174 136, 160 156, 146 164
           L 86 164
           C 72 154, 62 140, 46 124
           C 30 108, 48 86, 66 102 Z"
        fill="#FFFFFF" stroke="#000000" stroke-width="6.5" stroke-linejoin="round" stroke-linecap="round" />

  <path d="M 122 130 C 120 142, 118 150, 118 156"
        fill="none" stroke="#000000" stroke-width="5" stroke-linecap="round" />
  <path d="M 142 134 C 138 145, 132 152, 127 157"
        fill="none" stroke="#000000" stroke-width="5" stroke-linecap="round" />

  <rect x="76" y="158" width="80" height="30" rx="10"
        fill="#FFFFFF" stroke="#000000" stroke-width="6.5" stroke-linejoin="round" />

  <line x1="94" y1="166" x2="94" y2="180" stroke="#000000" stroke-width="5" stroke-linecap="round" />
  <line x1="109" y1="166" x2="109" y2="180" stroke="#000000" stroke-width="5" stroke-linecap="round" />
  <line x1="124" y1="166" x2="124" y2="180" stroke="#000000" stroke-width="5" stroke-linecap="round" />
  <line x1="139" y1="166" x2="139" y2="180" stroke="#000000" stroke-width="5" stroke-linecap="round" />
</svg>
"##;

pub struct Spring {
    pub val: f32,
    pub vel: f32,
    pub target: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl Spring {
    pub fn new(val: f32, stiffness: f32, damping: f32) -> Self {
        Self { val, vel: 0.0, target: val, stiffness, damping }
    }

    pub fn update(&mut self, dt: f32) {
        let force = (self.target - self.val) * self.stiffness;
        self.vel = (self.vel + force * dt) * (-self.damping * dt).exp();
        self.val += self.vel * dt;
    }
}

struct CartoonRay {
    angle: f32,
    angular_vel: f32,
    length_mult: f32,
    speed_mult: f32,
    delay: f32,
}

struct ImpactBurst {
    age: f32,
    max_age: f32,
    heat_at_spawn: f32,
    rays: Vec<CartoonRay>,
    arc_start_rad: f32,
    arc_sweep_rad: f32,
}

pub struct PokeAnimator {
    hand_pixmap: Pixmap,
    canvas: Pixmap,
    pub poke_spring: Spring,
    pub tilt_spring: Spring,
    pub scale_spring: Spring,
    pub combo_heat: f32,
    bursts: Vec<ImpactBurst>,
}

impl PokeAnimator {
    pub fn new() -> Self {
        let opt = Options::default();
        let tree = Tree::from_str(HAND_SVG, &opt).expect("Valid embedded SVG");
        let mut hand_pixmap = Pixmap::new(240, 320).unwrap();
        resvg::render(&tree, Transform::default(), &mut hand_pixmap.as_mut());

        Self {
            hand_pixmap,
            canvas: Pixmap::new(CANVAS_SIZE, CANVAS_SIZE).unwrap(),
            poke_spring: Spring::new(0.0, 480.0, 14.0),
            tilt_spring: Spring::new(0.0, 380.0, 12.0),
            scale_spring: Spring::new(BASE_HAND_SCALE, 300.0, 11.0),
            combo_heat: 0.0,
            bursts: Vec::with_capacity(16),
        }
    }

    pub fn trigger_click(&mut self, is_down: bool) {
        if is_down {
            self.combo_heat = (self.combo_heat + 1.2).min(12.0);
            let intensity = 1.0 + self.combo_heat * 0.18;

            self.poke_spring.val = -12.0 * intensity.min(2.2);
            self.poke_spring.vel = 620.0 * intensity;

            self.tilt_spring.val = 10.0 * intensity.min(2.0);
            self.tilt_spring.vel = -360.0 * intensity;

            self.scale_spring.vel += 2.2;
            self.spawn_impact_burst();
        } else {
            self.poke_spring.vel -= 140.0;
        }
    }

    fn spawn_impact_burst(&mut self) {
        let mut rng = rand::thread_rng();
        let num_rays = (5 + (self.combo_heat * 0.75) as usize).min(13);

        let start_deg = -235.0f32;
        let end_deg = 10.0f32;
        let span = end_deg - start_deg;

        let mut rays = Vec::with_capacity(num_rays);
        for i in 0..num_rays {
            let base_t = if num_rays > 1 {
                i as f32 / (num_rays - 1) as f32
            } else {
                0.5
            };
            let jitter = rng.gen_range(-7.5..7.5);
            let deg = start_deg + span * base_t + jitter;

            let is_primary = i % 2 == 0;
            let length_mult = if is_primary {
                rng.gen_range(1.15..1.45)
            } else {
                rng.gen_range(0.65..0.90)
            };

            rays.push(CartoonRay {
                angle: deg.to_radians(),
                angular_vel: rng.gen_range(-1.2..1.2),
                length_mult,
                speed_mult: rng.gen_range(0.85..1.20),
                delay: if is_primary { 0.0 } else { rng.gen_range(0.012..0.032) },
            });
        }

        let arc_center = (-112.0f32 + rng.gen_range(-20.0..20.0)).to_radians();
        let arc_sweep = rng.gen_range(55.0f32..95.0f32).to_radians();

        self.bursts.push(ImpactBurst {
            age: 0.0,
            max_age: (0.19 + self.combo_heat * 0.012).min(0.32),
            heat_at_spawn: self.combo_heat,
            rays,
            arc_start_rad: arc_center - arc_sweep * 0.5,
            arc_sweep_rad: arc_sweep,
        });
    }

    pub fn step(&mut self, dt: f32, user_scale: f32) {
        self.combo_heat *= (-dt * 1.6).exp();
        if self.combo_heat < 0.05 {
            self.combo_heat = 0.0;
        }

        let raw_target = (BASE_HAND_SCALE + self.combo_heat * 0.045) * user_scale;
        self.scale_spring.target = raw_target.min(1.15);

        self.poke_spring.update(dt);
        self.tilt_spring.update(dt);
        self.scale_spring.update(dt);

        for b in &mut self.bursts {
            b.age += dt;
        }
        self.bursts.retain(|b| b.age < b.max_age);
    }

    fn draw_impact_bursts(&mut self, effect_scale: f32) {
        let cx = HOTSPOT_X as f32;
        let cy = HOTSPOT_Y as f32;

        for b in &self.bursts {
            let heat_factor = 1.0 + b.heat_at_spawn * 0.24;

            let (r_col, g_col, b_col) = if b.heat_at_spawn < 2.0 {
                (255, 255, 255)
            } else if b.heat_at_spawn < 6.0 {
                let t = ((b.heat_at_spawn - 2.0) / 4.0).clamp(0.0, 1.0);
                (255, (255.0 - t * 30.0) as u8, (255.0 - t * 195.0) as u8)
            } else {
                let t = ((b.heat_at_spawn - 6.0) / 6.0).clamp(0.0, 1.0);
                (255, (225.0 - t * 85.0) as u8, (60.0 - t * 40.0) as u8)
            };

            let mut rays_pb = PathBuilder::new();

            for ray in &b.rays {
                let local_age = b.age - ray.delay;
                if local_age <= 0.0 {
                    continue;
                }
                let local_max = b.max_age - ray.delay;
                let p = (local_age / local_max).clamp(0.0, 1.0);

                let ease_out = 1.0 - (1.0 - p).powi(3);
                let pop_env = (p * 5.0).min(1.0) * (1.0 - p.powi(3));

                let cur_angle = ray.angle + ray.angular_vel * local_age;
                let dir_x = cur_angle.cos();
                let dir_y = cur_angle.sin();
                let perp_x = -dir_y;
                let perp_y = dir_x;

                let travel = (12.0 + ease_out * 28.0 * heat_factor * ray.speed_mult) * effect_scale;
                let base_len = (13.0 + b.heat_at_spawn * 2.6) * ray.length_mult * pop_env * effect_scale;

                if base_len < 1.5 {
                    continue;
                }

                let r_in = travel;
                let r_out = travel + base_len;
                let head_w = (2.6 + b.heat_at_spawn * 0.34).min(6.8) * pop_env * effect_scale;

                let tail_x = cx + dir_x * r_in;
                let tail_y = cy + dir_y * r_in;

                let head_left_x = cx + dir_x * r_out + perp_x * head_w;
                let head_left_y = cy + dir_y * r_out + perp_y * head_w;

                let head_right_x = cx + dir_x * r_out - perp_x * head_w;
                let head_right_y = cy + dir_y * r_out - perp_y * head_w;

                let tip_ctrl_x = cx + dir_x * (r_out + head_w * 1.45);
                let tip_ctrl_y = cy + dir_y * (r_out + head_w * 1.45);

                rays_pb.move_to(tail_x, tail_y);
                rays_pb.line_to(head_left_x, head_left_y);
                rays_pb.quad_to(tip_ctrl_x, tip_ctrl_y, head_right_x, head_right_y);
                rays_pb.close();
            }

            if let Some(rays_path) = rays_pb.finish() {
                let mut fill_paint = Paint::default();
                fill_paint.set_color_rgba8(r_col, g_col, b_col, 255);
                fill_paint.anti_alias = true;

                let mut outline_paint = Paint::default();
                outline_paint.set_color_rgba8(0, 0, 0, 255);
                outline_paint.anti_alias = true;

                let outline_stroke = Stroke {
                    width: 4.5 * effect_scale,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Default::default()
                };

                self.canvas.stroke_path(
                    &rays_path,
                    &outline_paint,
                    &outline_stroke,
                    Transform::default(),
                    None,
                );
                self.canvas.fill_path(
                    &rays_path,
                    &fill_paint,
                    FillRule::Winding,
                    Transform::default(),
                    None,
                );
            }

            if b.heat_at_spawn >= 2.2 {
                let p = (b.age / b.max_age).clamp(0.0, 1.0);
                let ease_out = 1.0 - (1.0 - p).powi(3);
                let arc_pop = (p * 4.5).min(1.0) * (1.0 - p.powi(2));

                if arc_pop > 0.1 {
                    let arc_r = (10.0 + ease_out * 22.0 * heat_factor) * effect_scale;
                    let current_sweep = b.arc_sweep_rad * arc_pop;
                    let center_angle = b.arc_start_rad + b.arc_sweep_rad * 0.5;
                    let a0 = center_angle - current_sweep * 0.5;

                    let mut arc_pb = PathBuilder::new();
                    let steps = 10;
                    for s in 0..=steps {
                        let a = a0 + current_sweep * (s as f32 / steps as f32);
                        let px = cx + a.cos() * arc_r;
                        let py = cy + a.sin() * arc_r;
                        if s == 0 {
                            arc_pb.move_to(px, py);
                        } else {
                            arc_pb.line_to(px, py);
                        }
                    }

                    if let Some(arc_path) = arc_pb.finish() {
                        let arc_thick = (2.5 + b.heat_at_spawn * 0.25).min(5.5) * arc_pop * effect_scale;

                        let mut outline_paint = Paint::default();
                        outline_paint.set_color_rgba8(0, 0, 0, 255);
                        outline_paint.anti_alias = true;

                        let mut inner_paint = Paint::default();
                        inner_paint.set_color_rgba8(r_col, g_col, b_col, 255);
                        inner_paint.anti_alias = true;

                        self.canvas.stroke_path(
                            &arc_path,
                            &outline_paint,
                            &Stroke {
                                width: arc_thick + 3.8 * effect_scale,
                                line_cap: LineCap::Round,
                                line_join: LineJoin::Round,
                                ..Default::default()
                            },
                            Transform::default(),
                            None,
                        );
                        self.canvas.stroke_path(
                            &arc_path,
                            &inner_paint,
                            &Stroke {
                                width: arc_thick,
                                line_cap: LineCap::Round,
                                line_join: LineJoin::Round,
                                ..Default::default()
                            },
                            Transform::default(),
                            None,
                        );
                    }
                }
            }
        }
    }

    pub fn render_bgra(&mut self, dpi_scale: f32, user_scale: f32, presentation_mode: bool) -> &[u8] {
        self.canvas.fill(Color::TRANSPARENT);

        let effect_scale = (dpi_scale * user_scale.sqrt()).clamp(0.6, 2.5);
        self.draw_impact_bursts(effect_scale);

        let total_scale = (self.scale_spring.val * dpi_scale).clamp(0.20, 1.45);
        let poke_dist = self.poke_spring.val * effect_scale;
        let angle_deg = BASE_ANGLE_DEG + self.tilt_spring.val.clamp(-35.0, 35.0);

        let rad = BASE_ANGLE_DEG.to_radians();
        let dir_x = rad.sin();
        let dir_y = -rad.cos();

        let transform = Transform::from_translate(-SVG_TIP_X, -SVG_TIP_Y)
            .post_scale(total_scale, total_scale)
            .post_rotate(angle_deg)
            .post_translate(
                HOTSPOT_X as f32 + poke_dist * dir_x,
                HOTSPOT_Y as f32 + poke_dist * dir_y,
            );

        let paint = PixmapPaint {
            opacity: 1.0,
            blend_mode: tiny_skia::BlendMode::SourceOver,
            quality: tiny_skia::FilterQuality::Bilinear,
        };

        self.canvas
            .draw_pixmap(0, 0, self.hand_pixmap.as_ref(), &paint, transform, None);

        if presentation_mode {
            if let Some(path) = PathBuilder::from_circle(
                HOTSPOT_X as f32,
                HOTSPOT_Y as f32,
                8.0 * effect_scale,
            ) {
                let mut stroke_paint = Paint::default();
                stroke_paint.set_color_rgba8(255, 65, 54, 220);
                stroke_paint.anti_alias = true;
                let stroke = Stroke {
                    width: 2.5 * effect_scale,
                    ..Default::default()
                };
                self.canvas
                    .stroke_path(&path, &stroke_paint, &stroke, Transform::default(), None);
            }
        }

        let data = self.canvas.data_mut();
        for pixel in data.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        self.canvas.data()
    }
}