const GLOVE_PATH: &str = "M 76 108 L 76 34 C 76 14, 104 14, 104 34 L 104 88 C 106 72, 128 72, 130 88 C 134 76, 154 78, 154 94 C 158 84, 176 90, 174 108 C 174 136, 160 156, 146 164 L 86 164 C 72 154, 62 140, 46 124 C 30 108, 48 86, 66 102 Z";
const KNUCKLE_LEFT: &str = "M 122 130 C 120 142, 118 150, 118 156";
const KNUCKLE_RIGHT: &str = "M 142 134 C 138 145, 132 152, 127 157";
const GINGERBREAD_PATH: &str = "M 76 108 L 76 34 C 76 14, 104 14, 104 34 L 104 88 C 106 72, 128 72, 130 88 C 134 76, 154 78, 154 94 C 158 84, 176 90, 174 108 C 174 144, 166 174, 156 190 C 150 198, 139 200, 126 200 L 90 200 C 76 200, 70 189, 72 173 C 64 160, 52 148, 42 132 C 28 114, 26 96, 42 90 C 52 86, 62 96, 70 106 Z";
const CUFF_SQUIGGLE: &str = "M 86 174 Q 93 165, 100 174 T 114 174 T 128 174 T 142 174 T 150 172";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HandStyle(pub u8);

impl HandStyle {
    pub fn from_index(index: u8) -> Self {
        if (index as usize) < HAND_STYLES.len() {
            Self(index)
        } else {
            Self::default()
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        HAND_STYLES
            .iter()
            .position(|binding| binding.id == id)
            .map(|index| Self(index as u8))
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn binding(self) -> &'static HandStyleBinding {
        &HAND_STYLES[self.index()]
    }
}

pub struct HandStyleBinding {
    pub id: &'static str,
    pub label: &'static str,
    pub markup: fn(f32, f32, f32) -> String,
}

pub const HAND_STYLES: &[HandStyleBinding] = &[
    HandStyleBinding {
        id: "classic",
        label: "Classic",
        markup: glove_markup,
    },
    HandStyleBinding {
        id: "gingerbread",
        label: "Gingerbread",
        markup: gingerbread_markup,
    },
];

pub fn hand_markup(style: HandStyle, outline: f32, knuckle: f32, finger: f32) -> String {
    (style.binding().markup)(outline, knuckle, finger)
}

fn finger_lines(finger: f32, stroke: &str) -> String {
    [94, 109, 124, 139]
        .iter()
        .map(|x| {
            format!(
                r##"<line x1="{x}" y1="166" x2="{x}" y2="180" stroke="{stroke}" stroke-width="{finger}" stroke-linecap="round" />"##
            )
        })
        .collect::<String>()
}

pub fn glove_markup(outline: f32, knuckle: f32, finger: f32) -> String {
    let path = GLOVE_PATH;
    let left = KNUCKLE_LEFT;
    let right = KNUCKLE_RIGHT;
    let fingers = finger_lines(finger, "#000000");
    format!(
        r##"<path d="{path}" fill="#FFFFFF" stroke="#000000" stroke-width="{outline}" stroke-linejoin="round" stroke-linecap="round" />
<path d="{left}" fill="none" stroke="#000000" stroke-width="{knuckle}" stroke-linecap="round" />
<path d="{right}" fill="none" stroke="#000000" stroke-width="{knuckle}" stroke-linecap="round" />
<rect x="76" y="158" width="80" height="30" rx="10" fill="#FFFFFF" stroke="#000000" stroke-width="{outline}" stroke-linejoin="round" />
{fingers}"##
    )
}

pub fn gingerbread_markup(outline: f32, _knuckle: f32, _finger: f32) -> String {
    let path = GINGERBREAD_PATH;
    let squiggle = CUFF_SQUIGGLE;
    let ink = outline + 8.5;
    format!(
        r##"<defs><clipPath id="cookie-clip"><path d="{path}" /></clipPath></defs>
<path d="{path}" fill="#C19A6B" stroke="#000000" stroke-width="{ink}" stroke-linejoin="round" stroke-linecap="round" />
<path d="{path}" fill="none" stroke="#FFFFFF" stroke-width="11" stroke-linejoin="round" stroke-linecap="round" clip-path="url(#cookie-clip)" />
<circle cx="106" cy="116" r="4.5" fill="#FF4136" />
<circle cx="127" cy="116" r="4.5" fill="#2ECC40" />
<circle cx="148" cy="119" r="4.5" fill="#0074D9" />
<path d="{squiggle}" fill="none" stroke="#FFFFFF" stroke-width="5.5" stroke-linecap="round" stroke-linejoin="round" />"##
    )
}

pub fn glove_silhouette() -> String {
    let path = GLOVE_PATH;
    format!(
        r##"<path d="{path}" fill="#000000" />
<rect x="76" y="158" width="80" height="30" rx="10" fill="#000000" />"##
    )
}

pub fn gingerbread_silhouette() -> String {
    format!(
        r##"<path d="{path}" fill="#000000" />"##,
        path = GINGERBREAD_PATH
    )
}

pub fn hand_silhouette(style: HandStyle) -> String {
    match style {
        HandStyle(0) => glove_silhouette(),
        _ => gingerbread_silhouette(),
    }
}

#[cfg(test)]
mod tests {
    use super::{gingerbread_markup, glove_markup, hand_markup, HandStyle, HAND_STYLES};

    #[test]
    fn registry_ids_are_unique_and_append_only() {
        let ids: Vec<&str> = HAND_STYLES.iter().map(|binding| binding.id).collect();
        assert_eq!(ids, vec!["classic", "gingerbread"]);
        assert_eq!(HandStyle::from_id("gingerbread"), Some(HandStyle(1)));
        assert_eq!(HandStyle::from_id("unknown"), None);
        assert_eq!(HandStyle::from_index(9), HandStyle::default());
    }

    #[test]
    fn gingerbread_uses_the_same_geometry() {
        let classic = glove_markup(8.5, 6.5, 6.0);
        let gingerbread = gingerbread_markup(8.5, 6.5, 6.0);
        let path = "M 76 108 L 76 34";
        assert!(classic.contains(path));
        assert!(gingerbread.contains(path));
        assert!(gingerbread.contains("#C19A6B"));
        assert!(gingerbread.contains("stroke=\"#FFFFFF\""));
        assert_eq!(gingerbread.matches("<circle").count(), 3);
        assert!(gingerbread.contains("clip-path=\"url(#cookie-clip)\""));
        assert!(!gingerbread.contains("<rect"));
    }

    #[test]
    fn hand_markup_dispatches_to_the_registered_function() {
        assert_eq!(
            hand_markup(HandStyle(0), 6.5, 5.0, 5.0),
            glove_markup(6.5, 5.0, 5.0)
        );
    }
}
