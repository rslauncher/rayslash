use std::{path::Path, sync::OnceLock};

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

pub(crate) const MAX_BITMAP_ICON_EDGE: u32 = 256;
pub(crate) const RESULT_APP_ICON_EDGE: u32 = 40;
pub(crate) const RESULT_FAVICON_EDGE: u32 = 32;
pub(crate) const ALTERNATE_OPENER_ICON_EDGE: u32 = 28;
pub(crate) const SETTINGS_FAVICON_EDGE: u32 = 28;
pub(crate) const SETTINGS_APP_CHOICE_ICON_EDGE: u32 = 22;

const MAX_DECODE_ALLOC_BYTES: u64 = 32 * 1024 * 1024;
// Keep at least 4x oversampling before the final Lanczos3 pass so oversized
// 512-1024px icons avoid large temporary f32 buffers without losing detail.
const PRESHRINK_OVERSAMPLE_FACTOR: u32 = 4;

pub(crate) fn physical_icon_edge(logical_edge: u32, scale_factor: f32) -> u32 {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    (logical_edge as f32 * scale)
        .round()
        .clamp(1.0, MAX_BITMAP_ICON_EDGE as f32) as u32
}

pub(crate) fn load_image(path: &Path, logical_edge: u32, scale_factor: f32) -> Option<Image> {
    load_bitmap_at_physical_edge(path, physical_icon_edge(logical_edge, scale_factor))
}

pub(crate) fn load_bitmap_at_physical_edge(path: &Path, physical_edge: u32) -> Option<Image> {
    let mut reader = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_DECODE_ALLOC_BYTES);
    reader.limits(limits);
    let source = reader.decode().ok()?.into_rgba8();
    Some(to_slint_image(source, physical_edge))
}

fn to_slint_image(source: image::RgbaImage, physical_edge: u32) -> Image {
    // Slint's software renderer uses nearest-neighbour sampling even for a
    // smooth Image. Pre-filter bitmap icons once at their exact physical size
    // so the software renderer blits 1:1 physical pixels without blockiness.
    let (width, height) = target_dimensions(source.dimensions(), physical_edge);
    if source.dimensions() == (width, height) {
        return Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            source.as_raw(),
            width,
            height,
        ));
    }

    Image::from_rgba8_premultiplied(resize_premultiplied_linear(&source, width, height))
}

fn target_dimensions((src_width, src_height): (u32, u32), physical_edge: u32) -> (u32, u32) {
    let edge = physical_edge.clamp(1, MAX_BITMAP_ICON_EDGE);
    let longest = src_width.max(src_height).max(1) as f64;
    let width = (src_width as f64 / longest * edge as f64).round().max(1.0) as u32;
    let height = (src_height as f64 / longest * edge as f64).round().max(1.0) as u32;
    (width, height)
}

fn resize_premultiplied_linear(
    source: &image::RgbaImage,
    width: u32,
    height: u32,
) -> SharedPixelBuffer<Rgba8Pixel> {
    let (src_width, src_height) = source.dimensions();
    let linear_premul = build_linear_premultiplied_source(source, width, height);

    // Lanczos3 preserves fine detail when downscaling; CatmullRom avoids
    // ringing halos when upscaling smaller source bitmaps.
    let filter = if width < src_width || height < src_height {
        image::imageops::FilterType::Lanczos3
    } else {
        image::imageops::FilterType::CatmullRom
    };
    let resized = image::imageops::resize(&linear_premul, width, height, filter);

    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    for (dst, src) in buffer.make_mut_slice().iter_mut().zip(resized.pixels()) {
        let alpha = src[3].clamp(0.0, 1.0);
        let alpha_u8 = (alpha * 255.0).round() as u8;
        if alpha_u8 == 0 || alpha <= 1e-6 {
            *dst = Rgba8Pixel::new(0, 0, 0, 0);
            continue;
        }
        let inv_alpha = 1.0 / alpha;
        let alpha_f = f32::from(alpha_u8);
        let r = (linear_to_srgb((src[0] * inv_alpha).clamp(0.0, 1.0)) * alpha_f)
            .round()
            .clamp(0.0, alpha_f) as u8;
        let g = (linear_to_srgb((src[1] * inv_alpha).clamp(0.0, 1.0)) * alpha_f)
            .round()
            .clamp(0.0, alpha_f) as u8;
        let b = (linear_to_srgb((src[2] * inv_alpha).clamp(0.0, 1.0)) * alpha_f)
            .round()
            .clamp(0.0, alpha_f) as u8;
        *dst = Rgba8Pixel::new(r, g, b, alpha_u8);
    }
    buffer
}

fn build_linear_premultiplied_source(
    source: &image::RgbaImage,
    target_width: u32,
    target_height: u32,
) -> image::Rgba32FImage {
    let lut = srgb_to_linear_lut();
    let (src_width, src_height) = source.dimensions();
    let mid_width = src_width.min(
        target_width
            .saturating_mul(PRESHRINK_OVERSAMPLE_FACTOR)
            .max(1),
    );
    let mid_height = src_height.min(
        target_height
            .saturating_mul(PRESHRINK_OVERSAMPLE_FACTOR)
            .max(1),
    );

    if (mid_width, mid_height) == (src_width, src_height) {
        let mut linear_premul = image::Rgba32FImage::new(src_width, src_height);
        for (dst, src) in linear_premul.pixels_mut().zip(source.pixels()) {
            let alpha = f32::from(src[3]) / 255.0;
            dst.0 = [
                lut[usize::from(src[0])] * alpha,
                lut[usize::from(src[1])] * alpha,
                lut[usize::from(src[2])] * alpha,
                alpha,
            ];
        }
        return linear_premul;
    }

    // Exact 2D area-weighted box integration directly into a 4x-oversampled
    // linear-light premultiplied buffer. Avoids allocating multi-megabyte f32
    // staging images for 512-1024px icons while feeding Lanczos3 clean signal.
    let mut mid = image::Rgba32FImage::new(mid_width, mid_height);
    let scale_x = src_width as f64 / mid_width as f64;
    let scale_y = src_height as f64 / mid_height as f64;

    for oy in 0..mid_height {
        let y0 = oy as f64 * scale_y;
        let y1 = (oy + 1) as f64 * scale_y;
        let sy_start = y0.floor() as u32;
        let sy_end = (y1.ceil() as u32).min(src_height);

        for ox in 0..mid_width {
            let x0 = ox as f64 * scale_x;
            let x1 = (ox + 1) as f64 * scale_x;
            let sx_start = x0.floor() as u32;
            let sx_end = (x1.ceil() as u32).min(src_width);

            let mut acc = [0.0f32; 4];
            let mut weight_sum = 0.0f32;

            for sy in sy_start..sy_end {
                let wy = (y1.min(f64::from(sy + 1)) - y0.max(f64::from(sy))) as f32;
                if wy <= 0.0 {
                    continue;
                }
                for sx in sx_start..sx_end {
                    let wx = (x1.min(f64::from(sx + 1)) - x0.max(f64::from(sx))) as f32;
                    if wx <= 0.0 {
                        continue;
                    }
                    let weight = wx * wy;
                    let pixel = source.get_pixel(sx, sy);
                    let alpha = f32::from(pixel[3]) / 255.0;
                    let weighted_alpha = alpha * weight;
                    acc[0] += lut[usize::from(pixel[0])] * weighted_alpha;
                    acc[1] += lut[usize::from(pixel[1])] * weighted_alpha;
                    acc[2] += lut[usize::from(pixel[2])] * weighted_alpha;
                    acc[3] += weighted_alpha;
                    weight_sum += weight;
                }
            }

            let inv_weight = if weight_sum > 0.0 {
                1.0 / weight_sum
            } else {
                0.0
            };
            mid.put_pixel(
                ox,
                oy,
                image::Rgba([
                    acc[0] * inv_weight,
                    acc[1] * inv_weight,
                    acc[2] * inv_weight,
                    acc[3] * inv_weight,
                ]),
            );
        }
    }

    mid
}

fn srgb_to_linear_lut() -> &'static [f32; 256] {
    static LUT: OnceLock<[f32; 256]> = OnceLock::new();
    LUT.get_or_init(|| {
        std::array::from_fn(|value| {
            let s = value as f32 / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        })
    })
}

fn linear_to_srgb(value: f32) -> f32 {
    let c = value.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn filtering_preserves_colour_at_transparent_edges() {
        let mut source = image::RgbaImage::from_pixel(144, 144, image::Rgba([0, 255, 0, 0]));
        for y in 24..120 {
            for x in 24..120 {
                source.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        let output = resize_premultiplied_linear(&source, 28, 28);
        let edges: Vec<_> = output
            .as_slice()
            .iter()
            .filter(|pixel| pixel.a > 10 && pixel.a < 245)
            .collect();
        assert!(!edges.is_empty(), "downscaled edges must be antialiased");
        assert!(
            edges
                .iter()
                .all(|pixel| pixel.r >= pixel.a.saturating_sub(2) && pixel.g == 0 && pixel.b == 0)
        );
    }

    #[test]
    fn fractional_scale_uses_physical_pixels_without_stretching() {
        for (scale, expected) in [
            (1.0, (28, 14)),
            (1.25, (35, 18)),
            (1.5, (42, 21)),
            (2.0, (56, 28)),
        ] {
            let edge = physical_icon_edge(28, scale);
            assert_eq!(target_dimensions((144, 72), edge), expected);
        }
    }

    #[test]
    fn linear_light_downscaling_preserves_midtone_luminance() {
        let mut source = image::RgbaImage::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                let value = if (x + y) % 2 == 0 { 255 } else { 0 };
                source.put_pixel(x, y, image::Rgba([value, value, value, 255]));
            }
        }
        let output = resize_premultiplied_linear(&source, 16, 16);
        let center = output.as_slice()[8 * 16 + 8];
        // 50% physical luminance encodes near 188 in sRGB (naive sRGB average is ~128).
        assert!(
            (180..=194).contains(&center.r),
            "expected ~188 linear-light midtone, got {}",
            center.r
        );
        assert_eq!(center.a, 255);
    }

    #[test]
    fn upscaling_small_icon_keeps_valid_premultiplied_pixels() {
        let mut source = image::RgbaImage::from_pixel(32, 32, image::Rgba([0, 0, 0, 0]));
        for y in 4..28 {
            for x in 4..28 {
                source.put_pixel(x, y, image::Rgba([40, 140, 240, 255]));
            }
        }
        let output = resize_premultiplied_linear(&source, 40, 40);
        assert_eq!((output.width(), output.height()), (40, 40));
        assert!(
            output
                .as_slice()
                .iter()
                .all(|pixel| pixel.r <= pixel.a && pixel.g <= pixel.a && pixel.b <= pixel.a)
        );
    }

    #[test]
    fn oversized_sources_preshrink_to_4x_oversampled_linear_buffer() {
        let mut source = image::RgbaImage::from_pixel(1024, 1024, image::Rgba([0, 255, 0, 0]));
        for y in 128..896 {
            for x in 128..896 {
                source.put_pixel(x, y, image::Rgba([255, 40, 20, 255]));
            }
        }
        let mid = build_linear_premultiplied_source(&source, 40, 40);
        assert_eq!(mid.dimensions(), (160, 160));

        let output = resize_premultiplied_linear(&source, 40, 40);
        assert_eq!((output.width(), output.height()), (40, 40));
        // Transparent green background must never bleed into antialiased edges.
        assert!(
            output
                .as_slice()
                .iter()
                .filter(|pixel| pixel.a > 10 && pixel.a < 245)
                .all(|pixel| pixel.r >= pixel.g && pixel.r <= pixel.a && pixel.g <= pixel.a)
        );
        let center = output.as_slice()[20 * 40 + 20];
        assert_eq!((center.r, center.g, center.b, center.a), (255, 40, 20, 255));
    }

    #[test]
    fn jpeg_icons_decode_and_resize_to_physical_edge() {
        let directory = std::env::temp_dir().join(format!(
            "rayslash-jpeg-icon-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("sample.jpg");
        image::RgbImage::from_pixel(128, 128, image::Rgb([60, 120, 200]))
            .save(&path)
            .unwrap();
        let loaded = load_image(&path, RESULT_APP_ICON_EDGE, 1.5).unwrap();
        assert_eq!((loaded.size().width, loaded.size().height), (60, 60));
        fs::remove_dir_all(directory).unwrap();
    }
}
