pub(crate) struct Spring {
    val: f32,
    vel: f32,
    target: f32,
    stiffness: f32,
    damping: f32,
}

impl Spring {
    pub(crate) fn new(val: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            val,
            vel: 0.0,
            target: val,
            stiffness,
            damping,
        }
    }

    pub(crate) fn value(&self) -> f32 {
        self.val
    }

    pub(crate) fn set(&mut self, val: f32, vel: f32) {
        self.val = val;
        self.vel = vel;
    }

    pub(crate) fn add_velocity(&mut self, delta: f32) {
        self.vel += delta;
    }

    pub(crate) fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub(crate) fn update(&mut self, dt: f32) {
        let force = (self.target - self.val) * self.stiffness;
        self.vel = (self.vel + force * dt) * (-self.damping * dt).exp();
        self.val += self.vel * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::Spring;

    #[test]
    fn spring_moves_toward_target() {
        let mut spring = Spring::new(0.0, 100.0, 10.0);
        spring.set_target(10.0);
        for _ in 0..60 {
            spring.update(0.016);
        }
        assert!((spring.value() - 10.0).abs() < 0.5);
    }
}
