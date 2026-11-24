const GLOVE_PATH: &str = "M 76 108 L 76 34 C 76 14, 104 14, 104 34 L 104 88 C 106 72, 128 72, 130 88 C 134 76, 154 78, 154 94 C 158 84, 176 90, 174 108 C 174 136, 160 156, 146 164 L 86 164 C 72 154, 62 140, 46 124 C 30 108, 48 86, 66 102 Z";
const KNUCKLE_LEFT: &str = "M 122 130 C 120 142, 118 150, 118 156";
const KNUCKLE_RIGHT: &str = "M 142 134 C 138 145, 132 152, 127 157";

pub fn glove_markup(outline: f32, knuckle: f32, finger: f32) -> String {
    let path = GLOVE_PATH;
    let left = KNUCKLE_LEFT;
    let right = KNUCKLE_RIGHT;
    let fingers = [94, 109, 124, 139]
        .iter()
        .map(|x| {
            format!(
                r##"<line x1="{x}" y1="166" x2="{x}" y2="180" stroke="#000000" stroke-width="{finger}" stroke-linecap="round" />"##
            )
        })
        .collect::<String>();

    format!(
        r##"<path d="{path}" fill="#FFFFFF" stroke="#000000" stroke-width="{outline}" stroke-linejoin="round" stroke-linecap="round" />
<path d="{left}" fill="none" stroke="#000000" stroke-width="{knuckle}" stroke-linecap="round" />
<path d="{right}" fill="none" stroke="#000000" stroke-width="{knuckle}" stroke-linecap="round" />
<rect x="76" y="158" width="80" height="30" rx="10" fill="#FFFFFF" stroke="#000000" stroke-width="{outline}" stroke-linejoin="round" />
{fingers}"##
    )
}

pub fn glove_silhouette() -> String {
    let path = GLOVE_PATH;
    format!(
        r##"<path d="{path}" fill="#000000" />
<rect x="76" y="158" width="80" height="30" rx="10" fill="#000000" />"##
    )
}
