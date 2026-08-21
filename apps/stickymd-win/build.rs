use std::env;
use std::fs;
use std::path::PathBuf;

const MANIFEST_FILE: &str = "StickyMD.manifest";
const REQUIRED_DPI_MARKER: &str = ">PerMonitorV2<";

fn main() {
    println!("cargo:rerun-if-changed={MANIFEST_FILE}");
    let manifest = fs::read_to_string(MANIFEST_FILE)
        .unwrap_or_else(|error| panic!("failed to read {MANIFEST_FILE}: {error}"));
    assert!(
        manifest.contains(REQUIRED_DPI_MARKER),
        "{MANIFEST_FILE} must declare PerMonitorV2 DPI awareness"
    );

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }

    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo always sets CARGO_MANIFEST_DIR"),
    );
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo always sets OUT_DIR"));
    let icon_path = output_dir.join("StickyMD.ico");
    fs::write(&icon_path, build_icon()).expect("failed to write deterministic application icon");

    let mut resource = winresource::WindowsResource::new();
    resource
        .set_manifest_file(manifest_dir.join(MANIFEST_FILE).to_string_lossy().as_ref())
        .set_icon(icon_path.to_string_lossy().as_ref())
        .set("ProductName", "StickyMD")
        .set("FileDescription", "StickyMD portable Markdown scratchpad")
        .set("OriginalFilename", "StickyMD.exe")
        .set("LegalCopyright", "Copyright (c) 2026 Develata");
    resource
        .compile()
        .expect("failed to compile StickyMD Windows resources");
}

fn build_icon() -> Vec<u8> {
    const SIDE: usize = 32;
    const PIXEL_BYTES: usize = SIDE * SIDE * 4;
    const MASK_BYTES: usize = SIDE * 4;
    const IMAGE_BYTES: usize = 40 + PIXEL_BYTES + MASK_BYTES;
    let mut icon = Vec::with_capacity(22 + IMAGE_BYTES);

    push_u16(&mut icon, 0);
    push_u16(&mut icon, 1);
    push_u16(&mut icon, 1);
    icon.extend_from_slice(&[SIDE as u8, SIDE as u8, 0, 0]);
    push_u16(&mut icon, 1);
    push_u16(&mut icon, 32);
    push_u32(&mut icon, IMAGE_BYTES as u32);
    push_u32(&mut icon, 22);

    push_u32(&mut icon, 40);
    push_i32(&mut icon, SIDE as i32);
    push_i32(&mut icon, (SIDE * 2) as i32);
    push_u16(&mut icon, 1);
    push_u16(&mut icon, 32);
    push_u32(&mut icon, 0);
    push_u32(&mut icon, PIXEL_BYTES as u32);
    icon.extend_from_slice(&[0; 16]);

    for output_y in 0..SIDE {
        let y = SIDE - 1 - output_y;
        for x in 0..SIDE {
            let outside_corner = (!(3..SIDE - 3).contains(&x) && !(3..SIDE - 3).contains(&y))
                && !matches!((x, y), (2, 2) | (2, 29) | (29, 2) | (29, 29));
            if outside_corner {
                icon.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let folded = x >= 23 && y <= 8 && x + y >= 31;
            let mark = sticky_s_mark(x, y);
            let [red, green, blue] = if mark {
                [72, 58, 35]
            } else if folded {
                [225, 176, 55]
            } else {
                [255, 224, 112]
            };
            icon.extend_from_slice(&[blue, green, red, 255]);
        }
    }
    icon.resize(22 + IMAGE_BYTES, 0);
    icon
}

fn sticky_s_mark(x: usize, y: usize) -> bool {
    const GLYPH: [&str; 7] = ["1111", "1000", "1000", "1111", "0001", "0001", "1111"];
    if !(8..20).contains(&x) || !(5..26).contains(&y) {
        return false;
    }
    let glyph_x = (x - 8) / 3;
    let glyph_y = (y - 5) / 3;
    GLYPH[glyph_y].as_bytes()[glyph_x] == b'1'
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    #[test]
    fn phase9_generated_icon_has_one_complete_32_bit_image() {
        let icon = super::build_icon();
        assert_eq!(&icon[0..6], &[0, 0, 1, 0, 1, 0]);
        assert_eq!(icon[6], 32);
        assert_eq!(icon[7], 32);
        assert_eq!(u32::from_le_bytes(icon[14..18].try_into().unwrap()), 4_264);
        assert_eq!(u32::from_le_bytes(icon[18..22].try_into().unwrap()), 22);
        assert_eq!(icon.len(), 4_286);
    }
}
