//! Validate the actual image bytes embedded by the G5 desktop harness.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#image-safety-limits

use stickymd_render::image::{
    EncodedImageFormat, decode_scaled_image, inspect_encoded_image, prepare_encoded_image,
};

#[test]
fn phase7_g5_images_decode_and_contain_visible_color_regions() {
    let fixtures: [(&str, EncodedImageFormat, &[u8]); 4] = [
        (
            "PNG",
            EncodedImageFormat::Png,
            include_bytes!("fixtures/qualification-images/g5.png"),
        ),
        (
            "JPEG",
            EncodedImageFormat::Jpeg,
            include_bytes!("fixtures/qualification-images/g5.jpg"),
        ),
        (
            "WebP",
            EncodedImageFormat::Webp,
            include_bytes!("fixtures/qualification-images/g5.webp"),
        ),
        (
            "GIF",
            EncodedImageFormat::Gif,
            include_bytes!("fixtures/qualification-images/g5.gif"),
        ),
    ];
    for (name, format, bytes) in fixtures {
        let metadata =
            inspect_encoded_image(bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(metadata.format, format, "{name}");
        assert_eq!((metadata.width, metadata.height), (96, 64), "{name}");
        let prepared =
            prepare_encoded_image(bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(prepared.bytes(), bytes, "{name} bytes must be preserved");
        let raster = decode_scaled_image(bytes, 96, 64)
            .unwrap_or_else(|error| panic!("{name} preview decode: {error}"));
        assert_eq!((raster.width, raster.height), (96, 64), "{name}");
        for (x, y, expected) in [
            (16, 16, [210_u8, 68, 54]),
            (64, 16, [48, 128, 190]),
            (16, 48, [50, 142, 99]),
            (64, 48, [235, 185, 55]),
        ] {
            let offset = (y * 96 + x) * 4;
            let actual = &raster.rgba[offset..offset + 4];
            for channel in 0..3 {
                // JPEG is lossy. Sample away from quadrant boundaries and allow
                // bounded codec rounding without accepting an empty/flat raster.
                assert!(
                    actual[channel].abs_diff(expected[channel]) <= 12,
                    "{name} at ({x},{y}): {actual:?}"
                );
            }
            assert_eq!(actual[3], 255, "{name} must have visible opaque pixels");
        }
    }
}
