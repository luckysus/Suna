//! Bounded input contract. Original samples are never overwritten or auto-scaled.
use super::*;
use image::{DynamicImage, ImageDecoder, ImageFormat};

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Gray16Mapping {
    FullRange,
    Window { lower: u16, upper: u16 },
}

// Only a single classic TIFF IFD is supported. Decoding remains the image crate's
// responsibility; this preflight prevents silently selecting a page/overview or
// interpreting WhiteIsZero/planar/color data differently in native and Pillow.
fn single_tiff(bytes: &[u8]) -> Result<(), String> {
    let fail = || "unsupported_tiff_layout".to_string();
    let little = match bytes.get(..4) {
        Some(b"II\x2a\0") => true,
        Some(b"MM\0\x2a") => false,
        _ => return Err(fail()),
    };
    let short = |at: usize| -> Result<u16, String> {
        let b: [u8; 2] = bytes
            .get(at..at.checked_add(2).ok_or_else(fail)?)
            .ok_or_else(fail)?
            .try_into()
            .unwrap();
        Ok(if little {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    };
    let long = |at: usize| -> Result<u32, String> {
        let b: [u8; 4] = bytes
            .get(at..at.checked_add(4).ok_or_else(fail)?)
            .ok_or_else(fail)?
            .try_into()
            .unwrap();
        Ok(if little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    };
    let start = long(4)? as usize;
    if start < 8 || start > bytes.len().saturating_sub(2) {
        return Err(fail());
    }
    let count = short(start)? as usize;
    if count > 512 || long(start.checked_add(2 + count * 12).ok_or_else(fail)?)? != 0 {
        return Err(fail());
    }
    let mut tags = std::collections::HashSet::new();
    let mut photometric = None;
    for i in 0..count {
        let entry = start + 2 + i * 12;
        let tag = short(entry)?;
        if !tags.insert(tag) || tag == 330 {
            return Err(fail());
        }
        if matches!(tag, 262 | 274 | 284) {
            if short(entry + 2)? != 3 || long(entry + 4)? != 1 {
                return Err(fail());
            }
            let value = short(entry + 8)?;
            match tag {
                262 => photometric = Some(value),
                274 if value != 1 => return Err("unsupported_tiff_orientation".into()),
                284 if value != 1 => return Err(fail()),
                _ => (),
            }
        }
    }
    if !matches!(photometric, Some(1 | 2)) {
        return Err(fail());
    }
    Ok(())
}

pub(super) fn decode(
    bytes: &[u8],
    mapping: Option<&Gray16Mapping>,
) -> Result<(DynamicImage, Value, &'static str), String> {
    let format = image::guess_format(bytes).map_err(|_| "image_decode_failed")?;
    let extension = match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Tiff => {
            single_tiff(bytes)?;
            "tif"
        }
        _ => return Err("supported_images_png_jpeg_tiff_only".into()),
    };
    let (w, h) = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| "image_decode_failed")?;
    if w == 0 || h == 0 || w as u64 * h as u64 > 16_000_000 || w > 8192 || h > 8192 {
        return Err("image_pixel_limit".into());
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| "image_decode_failed")?;
    let orientation = decoder
        .orientation()
        .map_err(|_| "image_decode_failed")?
        .to_exif();
    let original_color = decoder.original_color_type();
    if format == ImageFormat::Tiff
        && !matches!(
            original_color,
            image::ExtendedColorType::L8
                | image::ExtendedColorType::L16
                | image::ExtendedColorType::Rgb8
        )
    {
        return Err("unsupported_image_depth".into());
    }
    let raster = DynamicImage::from_decoder(decoder).map_err(|_| "image_decode_failed")?;
    let (raster, bit_depth, color_model, normalization) = match raster {
        DynamicImage::ImageLuma16(samples) => {
            let (mode, lower, upper) = match mapping.ok_or("gray16_mapping_required")? {
                Gray16Mapping::FullRange => ("full_range", 0u32, 65535u32),
                Gray16Mapping::Window { lower, upper } if lower < upper => {
                    ("window", *lower as u32, *upper as u32)
                }
                _ => return Err("invalid_gray16_window".into()),
            };
            let gray = image::GrayImage::from_fn(w, h, |x, y| {
                let value = (samples[(x, y)][0] as u32).clamp(lower, upper);
                image::Luma([((value - lower) * 255 / (upper - lower)) as u8])
            });
            (
                DynamicImage::ImageLuma8(gray),
                16,
                "gray",
                json!({"version":"linear-u16.v1","mode":mode,"lower":lower,"upper":upper,"rounding":"floor"}),
            )
        }
        raster
            if matches!(
                raster.color(),
                image::ColorType::L8
                    | image::ColorType::La8
                    | image::ColorType::Rgb8
                    | image::ColorType::Rgba8
            ) =>
        {
            let model = if matches!(raster.color(), image::ColorType::L8 | image::ColorType::La8) {
                "gray"
            } else {
                "rgb"
            };
            (raster, 8, model, Value::Null)
        }
        _ => return Err("unsupported_image_depth".into()),
    };
    let contract = json!({
        "version":"microstructure-input.v1", "format":match format {ImageFormat::Png=>"PNG",ImageFormat::Jpeg=>"JPEG",_=>"TIFF"},
        "decodedBitDepth":bit_depth, "colorModel":color_model, "width":w, "height":h,
        "sourceExifOrientation":orientation, "orientationPolicy":"ignore-exif",
        "coordinateSystem":"raw-raster", "alphaPolicy":"white-composite", "iccPolicy":"unmanaged",
        "grayscaleMapping":normalization
    });
    Ok((raster, contract, extension))
}
