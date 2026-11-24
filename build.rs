use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[allow(dead_code)]
#[path = "src/hand_art.rs"]
mod hand_art;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/hand_art.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let ico_path = out_dir.join("taptap_generated.ico");

    // 1. Render the 256x256 icon SVG into a PNG byte buffer using resvg + tiny-skia
    let opt = resvg::usvg::Options::default();
    let icon_svg = format!(
        r##"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256">
  <circle cx="128" cy="128" r="120" fill="#FF6B00" stroke="#1E1E1E" stroke-width="10" />
  <g transform="translate(128, 134) rotate(-22) scale(0.68) translate(-115, -110)">
    {}
  </g>
</svg>
"##,
        hand_art::glove_markup(8.5, 6.5, 6.0)
    );
    let tree = resvg::usvg::Tree::from_str(&icon_svg, &opt).expect("Valid icon SVG");
    let mut pixmap = tiny_skia::Pixmap::new(256, 256).expect("Failed to allocate icon pixmap");
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    let png_bytes = pixmap.encode_png().expect("Failed to encode PNG for ICO");

    // 2. Wrap the PNG payload in a standard Windows ICO container header (22 bytes)
    let mut ico_file = File::create(&ico_path).expect("Failed to create .ico file");
    let png_len = png_bytes.len() as u32;

    let mut header = Vec::with_capacity(22);
    // ICONDIR header (6 bytes)
    header.extend_from_slice(&0u16.to_le_bytes()); // Reserved (0)
    header.extend_from_slice(&1u16.to_le_bytes()); // Type (1 = Icon)
    header.extend_from_slice(&1u16.to_le_bytes()); // Image count (1)

    // ICONDIRENTRY (16 bytes)
    header.push(0); // Width (0 represents 256px in ICO format)
    header.push(0); // Height (0 represents 256px in ICO format)
    header.push(0); // Color palette count (0)
    header.push(0); // Reserved (0)
    header.extend_from_slice(&1u16.to_le_bytes()); // Color planes (1)
    header.extend_from_slice(&32u16.to_le_bytes()); // Bits per pixel (32-bit RGBA)
    header.extend_from_slice(&png_len.to_le_bytes()); // Size of PNG data in bytes
    header.extend_from_slice(&22u32.to_le_bytes()); // Offset to PNG data (6 + 16 = 22)

    ico_file.write_all(&header).unwrap();
    ico_file.write_all(&png_bytes).unwrap();

    // 3. Compile the generated .ico and version metadata into the Windows executable
    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().unwrap());
    res.set("ProductName", "TapTap");
    res.set("FileDescription", "TapTap Animated Desktop Cursor Overlay");
    res.set("LegalCopyright", "Copyright (c) 2026");
    res.compile().expect("Failed to compile Windows resources");
}