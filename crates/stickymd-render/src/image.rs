//! Bounded image inspection, normalization, decoding, and raster cache.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#image-safety-limits

use std::io::{BufRead, Cursor, Seek};
use std::sync::Arc;

use image::codecs::bmp::BmpDecoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use stickymd_core::{Hash32, ManagedAssetExtension, hash_bytes};
use thiserror::Error;

pub const MAX_ENCODED_IMAGE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_IMAGE_SIDE: u32 = 16_384;
pub const MAX_IMAGE_PIXELS: u64 = 40_000_000;
pub const IMAGE_CACHE_BUDGET_BYTES: usize = 16 * 1024 * 1024;
pub const IMAGE_CACHE_MAX_ENTRIES: usize = 512;
/// Deterministic per-entry estimate for key/value/hash-table metadata. The
/// separate entry cap bounds allocator overhead that cannot be measured exactly.
pub const IMAGE_CACHE_ENTRY_OVERHEAD_BYTES: usize = 128;

mod cache;
pub use cache::{DecodedImageCache, ImageCacheCounters, ImageCacheKey};

/// Read-only execution-domain source used by the preview worker. Implementors
/// may read local files but must never perform network I/O.
pub trait PreviewImageSource {
    fn inspect(&self, destination: &str) -> Result<Option<ImageMetadata>, String>;
    fn load(&self, destination: &str) -> Result<Option<Vec<u8>>, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncodedImageFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
    Bmp,
    Ico,
}

impl EncodedImageFormat {
    pub const fn managed_extension(self) -> Option<ManagedAssetExtension> {
        match self {
            Self::Png => Some(ManagedAssetExtension::Png),
            Self::Jpeg => Some(ManagedAssetExtension::Jpg),
            Self::Webp => Some(ManagedAssetExtension::Webp),
            Self::Gif => Some(ManagedAssetExtension::Gif),
            Self::Bmp | Self::Ico => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageMetadata {
    pub format: EncodedImageFormat,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedImage {
    bytes: Arc<[u8]>,
    extension: ManagedAssetExtension,
    hash: Hash32,
    width: u32,
    height: u32,
}

impl PreparedImage {
    /// Final encoded bytes. The hash and extension were derived from exactly
    /// this immutable allocation by the validated preparation path.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn extension(&self) -> ManagedAssetExtension {
        self.extension
    }

    pub const fn hash(&self) -> Hash32 {
        self.hash
    }

    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImageRaster {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}
impl DecodedImageRaster {
    pub fn byte_len(&self) -> usize {
        self.rgba.len()
    }
}

#[derive(Debug, Error)]
pub enum ImageAssetError {
    #[error("encoded image is {actual} bytes; limit is {limit} bytes")]
    EncodedTooLarge { actual: usize, limit: usize },
    #[error("unsupported or unrecognized image format")]
    UnsupportedFormat,
    #[error("image dimension {width}x{height} exceeds the safety limit")]
    DimensionsTooLarge { width: u32, height: u32 },
    #[error("decoded image byte count overflow")]
    DimensionOverflow,
    #[error("image decode failed: {0}")]
    Decode(image::ImageError),
    #[error("image input failed: {0}")]
    Io(std::io::Error),
    #[error("RGBA payload length does not match its dimensions")]
    InvalidRgbaLength,
}

pub fn inspect_encoded_image(bytes: &[u8]) -> Result<ImageMetadata, ImageAssetError> {
    check_encoded_size(bytes.len())?;
    inspect_image_reader(Cursor::new(bytes))
}

/// Inspect dimensions and format from a seekable buffered reader without
/// retaining or decoding the full image payload.
pub fn inspect_image_reader<R>(reader: R) -> Result<ImageMetadata, ImageAssetError>
where
    R: BufRead + Seek,
{
    let mut reader = ImageReader::new(reader)
        .with_guessed_format()
        .map_err(ImageAssetError::Io)?;
    let format = reader
        .format()
        .and_then(map_format)
        .ok_or(ImageAssetError::UnsupportedFormat)?;
    reader.limits(decode_limits());
    let mut decoder = reader.into_decoder().map_err(ImageAssetError::Decode)?;
    let (width, height) = decoder.dimensions();
    check_dimensions(width, height)?;
    let orientation = decoder.orientation().map_err(ImageAssetError::Decode)?;
    let (width, height) = oriented_dimensions(width, height, orientation);
    check_dimensions(width, height)?;
    Ok(ImageMetadata {
        format,
        width,
        height,
    })
}

/// Validate a clipboard/file image completely. Stable encoded formats retain
/// their exact bytes; BMP/ICO inputs are normalized to PNG.
pub fn prepare_encoded_image(bytes: &[u8]) -> Result<PreparedImage, ImageAssetError> {
    prepare_encoded_image_owned(bytes.to_vec())
}

/// Validate an owned clipboard/file image without cloning stable encoded
/// bytes. The worker already owns these payloads, so consuming them avoids a
/// second allocation of up to 64 MiB while the decoded validation raster is
/// also live.
pub fn prepare_encoded_image_owned(bytes: Vec<u8>) -> Result<PreparedImage, ImageAssetError> {
    let metadata = inspect_encoded_image(&bytes)?;
    let image = decode_encoded(&bytes)?;
    let (final_bytes, extension) = match metadata.format.managed_extension() {
        Some(extension) => (bytes, extension),
        None => (encode_png(&image)?, ManagedAssetExtension::Png),
    };
    Ok(PreparedImage {
        hash: hash_bytes(&final_bytes),
        bytes: final_bytes.into(),
        extension,
        width: image.width(),
        height: image.height(),
    })
}

/// Convert a Windows DIB/DIBV5 payload, which lacks a BMP file header, to PNG.
pub fn prepare_dib_image(bytes: &[u8]) -> Result<PreparedImage, ImageAssetError> {
    check_encoded_size(bytes.len())?;
    let mut decoder =
        BmpDecoder::new_without_file_header(Cursor::new(bytes)).map_err(ImageAssetError::Decode)?;
    let (width, height) = decoder.dimensions();
    check_dimensions(width, height)?;
    decoder
        .set_limits(decode_limits())
        .map_err(ImageAssetError::Decode)?;
    let image = DynamicImage::from_decoder(decoder).map_err(ImageAssetError::Decode)?;
    prepared_png(image)
}

pub fn prepare_rgba_image(
    width: u32,
    height: u32,
    rgba: Vec<u8>,
) -> Result<PreparedImage, ImageAssetError> {
    check_dimensions(width, height)?;
    if rgba.len() != decoded_len(width, height)? {
        return Err(ImageAssetError::InvalidRgbaLength);
    }
    let image = image::RgbaImage::from_raw(width, height, rgba)
        .map(DynamicImage::ImageRgba8)
        .ok_or(ImageAssetError::InvalidRgbaLength)?;
    prepared_png(image)
}

pub fn decode_scaled_image(
    bytes: &[u8],
    max_width: u32,
    max_height: u32,
) -> Result<DecodedImageRaster, ImageAssetError> {
    resize_decoded_image(decode_encoded(bytes)?, max_width, max_height)
}

/// Decode owned source bytes and release them before allocating the scaled
/// raster. Preview file reads already transfer ownership, so this avoids
/// retaining a large encoded BMP alongside both decoded image buffers.
pub fn decode_scaled_image_owned(
    bytes: Vec<u8>,
    max_width: u32,
    max_height: u32,
) -> Result<DecodedImageRaster, ImageAssetError> {
    let image = decode_encoded(&bytes)?;
    drop(bytes);
    resize_decoded_image(image, max_width, max_height)
}

fn resize_decoded_image(
    image: DynamicImage,
    max_width: u32,
    max_height: u32,
) -> Result<DecodedImageRaster, ImageAssetError> {
    let target = fit_without_upscale(
        image.width(),
        image.height(),
        max_width.max(1),
        max_height.max(1),
    );
    let image = if target == (image.width(), image.height()) {
        image
    } else {
        image.resize_exact(target.0, target.1, FilterType::Triangle)
    };
    let mut rgba = image.into_rgba8().into_raw();
    premultiply_rgba(&mut rgba);
    Ok(DecodedImageRaster {
        width: target.0,
        height: target.1,
        rgba: rgba.into(),
    })
}

fn decode_encoded(bytes: &[u8]) -> Result<DynamicImage, ImageAssetError> {
    check_encoded_size(bytes.len())?;
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(ImageAssetError::Io)?;
    if reader.format().and_then(map_format).is_none() {
        return Err(ImageAssetError::UnsupportedFormat);
    }
    reader.limits(decode_limits());
    let mut decoder = reader.into_decoder().map_err(ImageAssetError::Decode)?;
    let (width, height) = decoder.dimensions();
    check_dimensions(width, height)?;
    let orientation = decoder.orientation().map_err(ImageAssetError::Decode)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(ImageAssetError::Decode)?;
    image.apply_orientation(orientation);
    check_dimensions(image.width(), image.height())?;
    Ok(image)
}

fn prepared_png(image: DynamicImage) -> Result<PreparedImage, ImageAssetError> {
    check_dimensions(image.width(), image.height())?;
    let bytes = encode_png(&image)?;
    Ok(PreparedImage {
        hash: hash_bytes(&bytes),
        bytes: bytes.into(),
        extension: ManagedAssetExtension::Png,
        width: image.width(),
        height: image.height(),
    })
}

fn encode_png(image: &DynamicImage) -> Result<Vec<u8>, ImageAssetError> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(ImageAssetError::Decode)?;
    check_encoded_size(bytes.len())?;
    Ok(bytes)
}

fn decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(MAX_IMAGE_PIXELS.saturating_mul(4));
    limits
}

fn check_encoded_size(size: usize) -> Result<(), ImageAssetError> {
    if size > MAX_ENCODED_IMAGE_BYTES {
        Err(ImageAssetError::EncodedTooLarge {
            actual: size,
            limit: MAX_ENCODED_IMAGE_BYTES,
        })
    } else {
        Ok(())
    }
}

fn check_dimensions(width: u32, height: u32) -> Result<(), ImageAssetError> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(ImageAssetError::DimensionOverflow)?;
    if width == 0
        || height == 0
        || width > MAX_IMAGE_SIDE
        || height > MAX_IMAGE_SIDE
        || pixels > MAX_IMAGE_PIXELS
    {
        Err(ImageAssetError::DimensionsTooLarge { width, height })
    } else {
        Ok(())
    }
}

fn decoded_len(width: u32, height: u32) -> Result<usize, ImageAssetError> {
    usize::try_from(
        u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|n| n.checked_mul(4))
            .ok_or(ImageAssetError::DimensionOverflow)?,
    )
    .map_err(|_| ImageAssetError::DimensionOverflow)
}

fn map_format(format: ImageFormat) -> Option<EncodedImageFormat> {
    match format {
        ImageFormat::Png => Some(EncodedImageFormat::Png),
        ImageFormat::Jpeg => Some(EncodedImageFormat::Jpeg),
        ImageFormat::WebP => Some(EncodedImageFormat::Webp),
        ImageFormat::Gif => Some(EncodedImageFormat::Gif),
        ImageFormat::Bmp => Some(EncodedImageFormat::Bmp),
        ImageFormat::Ico => Some(EncodedImageFormat::Ico),
        _ => None,
    }
}

fn fit_without_upscale(width: u32, height: u32, max_width: u32, max_height: u32) -> (u32, u32) {
    if width <= max_width && height <= max_height {
        return (width, height);
    }
    let scale = (max_width as f64 / width as f64).min(max_height as f64 / height as f64);
    (
        ((width as f64 * scale).floor() as u32).max(1),
        ((height as f64 * scale).floor() as u32).max(1),
    )
}

const fn oriented_dimensions(width: u32, height: u32, orientation: Orientation) -> (u32, u32) {
    match orientation {
        Orientation::Rotate90
        | Orientation::Rotate270
        | Orientation::Rotate90FlipH
        | Orientation::Rotate270FlipH => (height, width),
        Orientation::NoTransforms
        | Orientation::Rotate180
        | Orientation::FlipHorizontal
        | Orientation::FlipVertical => (width, height),
    }
}

fn premultiply_rgba(bytes: &mut [u8]) {
    for pixel in bytes.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        pixel[0] = ((u16::from(pixel[0]) * alpha + 127) / 255) as u8;
        pixel[1] = ((u16::from(pixel[1]) * alpha + 127) / 255) as u8;
        pixel[2] = ((u16::from(pixel[2]) * alpha + 127) / 255) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rgba(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::with_capacity(decoded_len(width, height).unwrap());
        for _ in 0..u64::from(width) * u64::from(height) {
            out.extend_from_slice(&color);
        }
        out
    }
    #[test]
    fn bitmap_normalizes_to_valid_png() {
        let image = prepare_rgba_image(3, 2, rgba(3, 2, [10, 20, 30, 128])).unwrap();
        let metadata = inspect_encoded_image(&image.bytes).unwrap();
        assert_eq!((metadata.width, metadata.height), (3, 2));
        assert_eq!(metadata.format, EncodedImageFormat::Png);
    }
    #[test]
    fn preserved_png_keeps_exact_bytes() {
        let original = prepare_rgba_image(2, 2, rgba(2, 2, [1, 2, 3, 255])).unwrap();
        let prepared = prepare_encoded_image(&original.bytes).unwrap();
        assert_eq!(&*prepared.bytes, &*original.bytes);
    }

    #[test]
    fn owned_stable_format_keeps_exact_input_bytes() {
        let original = prepare_rgba_image(2, 2, rgba(2, 2, [1, 2, 3, 255])).unwrap();
        let bytes = original.bytes().to_vec();
        let prepared = prepare_encoded_image_owned(bytes.clone()).unwrap();
        assert_eq!(prepared.bytes(), bytes);
    }
    #[test]
    fn phase7_stable_formats_are_preserved_and_bmp_ico_normalize_to_png() {
        let source = DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(2, 2, rgba(2, 2, [10, 20, 30, 255])).unwrap(),
        );
        for (format, expected) in [
            (ImageFormat::Jpeg, ManagedAssetExtension::Jpg),
            (ImageFormat::WebP, ManagedAssetExtension::Webp),
            (ImageFormat::Gif, ManagedAssetExtension::Gif),
        ] {
            let mut bytes = Vec::new();
            source
                .write_to(&mut Cursor::new(&mut bytes), format)
                .unwrap();
            let prepared = prepare_encoded_image(&bytes).unwrap();
            assert_eq!(prepared.extension, expected);
            assert_eq!(&*prepared.bytes, bytes.as_slice());
        }
        for format in [ImageFormat::Bmp, ImageFormat::Ico] {
            let mut bytes = Vec::new();
            source
                .write_to(&mut Cursor::new(&mut bytes), format)
                .unwrap();
            let prepared = prepare_encoded_image(&bytes).unwrap();
            assert_eq!(prepared.extension, ManagedAssetExtension::Png);
            assert_eq!(
                inspect_encoded_image(&prepared.bytes).unwrap().format,
                EncodedImageFormat::Png
            );
        }
    }
    #[test]
    fn phase7_dib_and_alpha_decode_are_safe_and_premultiplied() {
        let mut dib_bytes = vec![0_u8; 44];
        dib_bytes[0..4].copy_from_slice(&40_u32.to_le_bytes());
        dib_bytes[4..8].copy_from_slice(&1_i32.to_le_bytes());
        dib_bytes[8..12].copy_from_slice(&1_i32.to_le_bytes());
        dib_bytes[12..14].copy_from_slice(&1_u16.to_le_bytes());
        dib_bytes[14..16].copy_from_slice(&32_u16.to_le_bytes());
        dib_bytes[20..24].copy_from_slice(&4_u32.to_le_bytes());
        dib_bytes[40..44].copy_from_slice(&[50, 100, 200, 255]);
        let dib = prepare_dib_image(&dib_bytes).unwrap();
        assert_eq!(dib.extension, ManagedAssetExtension::Png);
        let alpha = prepare_rgba_image(1, 1, rgba(1, 1, [200, 100, 50, 128])).unwrap();
        let decoded = decode_scaled_image(&alpha.bytes, 1, 1).unwrap();
        assert_eq!(decoded.rgba[3], 128);
        assert!(decoded.rgba[0] <= 101);
        assert!(decoded.rgba[1] <= 51);
        assert!(decoded.rgba[2] <= 26);
    }
    #[test]
    fn phase7_corrupt_and_all_size_limits_fail_closed() {
        assert!(matches!(
            prepare_encoded_image(b"not an image"),
            Err(ImageAssetError::UnsupportedFormat)
        ));
        assert!(matches!(
            check_encoded_size(MAX_ENCODED_IMAGE_BYTES + 1),
            Err(ImageAssetError::EncodedTooLarge { .. })
        ));
        assert!(matches!(
            check_dimensions(MAX_IMAGE_SIDE, MAX_IMAGE_SIDE),
            Err(ImageAssetError::DimensionsTooLarge { .. })
        ));
        assert!(decoded_len(u32::MAX, u32::MAX).is_err());
    }
    #[test]
    fn oversized_dimensions_fail_before_allocation() {
        assert!(matches!(
            prepare_rgba_image(MAX_IMAGE_SIDE + 1, 1, Vec::new()),
            Err(ImageAssetError::DimensionsTooLarge { .. })
        ));
        assert!(matches!(
            check_dimensions(10_000, 10_000),
            Err(ImageAssetError::DimensionsTooLarge { .. })
        ));
    }
    #[test]
    fn scaled_decode_never_upscales() {
        let prepared = prepare_rgba_image(100, 50, rgba(100, 50, [2, 4, 8, 255])).unwrap();
        let small = decode_scaled_image(&prepared.bytes, 20, 20).unwrap();
        assert_eq!((small.width, small.height), (20, 10));
        let same = decode_scaled_image(&prepared.bytes, 200, 200).unwrap();
        assert_eq!((same.width, same.height), (100, 50));
    }

    #[test]
    fn phase9_owned_decode_matches_borrowed_decode() {
        let prepared = prepare_rgba_image(100, 50, rgba(100, 50, [2, 4, 8, 128])).unwrap();
        let borrowed = decode_scaled_image(&prepared.bytes, 20, 20).unwrap();
        let owned = decode_scaled_image_owned(prepared.bytes().to_vec(), 20, 20).unwrap();

        assert_eq!(owned, borrowed);
    }
    #[test]
    fn orientation_metadata_matches_post_decode_dimensions() {
        for orientation in [
            Orientation::Rotate90,
            Orientation::Rotate270,
            Orientation::Rotate90FlipH,
            Orientation::Rotate270FlipH,
        ] {
            assert_eq!(oriented_dimensions(640, 480, orientation), (480, 640));
        }
        for orientation in [
            Orientation::NoTransforms,
            Orientation::Rotate180,
            Orientation::FlipHorizontal,
            Orientation::FlipVertical,
        ] {
            assert_eq!(oriented_dimensions(640, 480, orientation), (640, 480));
        }
    }
    #[test]
    fn jpeg_exif_orientation_is_applied_to_metadata_and_raster() {
        let source = DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(3, 2, rgba(3, 2, [10, 20, 30, 255])).unwrap(),
        );
        let mut jpeg = Vec::new();
        source
            .write_to(&mut Cursor::new(&mut jpeg), ImageFormat::Jpeg)
            .unwrap();

        // APP1 Exif with one little-endian SHORT orientation entry (= 6,
        // rotate 90 degrees clockwise). Keeping this fixture inline proves the
        // real decoder path instead of only testing our enum mapping.
        let mut exif =
            b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0".to_vec();
        let segment_len = u16::try_from(exif.len() + 2).unwrap().to_be_bytes();
        let mut oriented = Vec::with_capacity(jpeg.len() + exif.len() + 4);
        oriented.extend_from_slice(&jpeg[..2]);
        oriented.extend_from_slice(&[0xff, 0xe1]);
        oriented.extend_from_slice(&segment_len);
        oriented.append(&mut exif);
        oriented.extend_from_slice(&jpeg[2..]);

        let metadata = inspect_encoded_image(&oriented).unwrap();
        assert_eq!((metadata.width, metadata.height), (2, 3));
        let raster = decode_scaled_image(&oriented, 100, 100).unwrap();
        assert_eq!((raster.width, raster.height), (2, 3));
    }
    #[test]
    #[ignore = "Release-only Phase 7 image decode timing baseline"]
    fn phase7_image_decode_release_baseline() {
        use std::time::{Duration, Instant};

        let prepared = prepare_rgba_image(1024, 768, rgba(1024, 768, [20, 40, 80, 220])).unwrap();
        let mut inspect_samples = Vec::new();
        let mut decode_resize_samples = Vec::new();
        let mut cache_samples = Vec::new();
        for _ in 0..30 {
            let inspect_started = Instant::now();
            let metadata = inspect_encoded_image(&prepared.bytes).unwrap();
            inspect_samples.push(inspect_started.elapsed());
            assert_eq!((metadata.width, metadata.height), (1024, 768));

            let started = Instant::now();
            let raster = decode_scaled_image(&prepared.bytes, 800, 600).unwrap();
            decode_resize_samples.push(started.elapsed());
            assert_eq!((raster.width, raster.height), (800, 600));

            let mut cache = DecodedImageCache::default();
            let cache_started = Instant::now();
            assert!(
                cache
                    .insert(
                        ImageCacheKey {
                            source_hash: prepared.hash,
                            width: 800,
                            height: 600,
                        },
                        raster,
                    )
                    .is_some()
            );
            cache_samples.push(cache_started.elapsed());
        }
        fn stats(samples: &mut [Duration]) -> (u128, u128, u128) {
            samples.sort_unstable();
            (
                samples[15].as_micros(),
                samples[28].as_micros(),
                samples[29].as_micros(),
            )
        }
        let inspect = stats(&mut inspect_samples);
        let decode_resize = stats(&mut decode_resize_samples);
        let cache = stats(&mut cache_samples);
        println!(
            "phase7 image 1024x768->800x600 inspect_median_us={} inspect_p95_us={} \
             inspect_max_us={} decode_resize_median_us={} decode_resize_p95_us={} \
             decode_resize_max_us={} cache_median_us={} cache_p95_us={} cache_max_us={}",
            inspect.0,
            inspect.1,
            inspect.2,
            decode_resize.0,
            decode_resize.1,
            decode_resize.2,
            cache.0,
            cache.1,
            cache.2
        );
    }
}
