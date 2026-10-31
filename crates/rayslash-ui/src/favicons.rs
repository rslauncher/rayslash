use std::path::Path;

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

pub(crate) fn load_image(path: &Path, logical_edge: u32, scale_factor: f32) -> Option<Image> {
    let mut reader = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(16 * 1024 * 1024);
    reader.limits(limits);
    let source = reader.decode().ok()?.into_rgba8();
    let pixels = resize(source, logical_edge, scale_factor);
    Some(Image::from_rgba8(
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            pixels.as_raw(),
            pixels.width(),
            pixels.height(),
        ),
    ))
}

fn resize(mut source: image::RgbaImage, logical_edge: u32, scale_factor: f32) -> image::RgbaImage {
    // Slint's software renderer uses nearest-neighbour sampling even for a
    // smooth Image. Filter these small icons once at their physical size.
    let edge = (logical_edge as f32 * scale_factor)
        .round()
        .clamp(1.0, 128.0) as u32;
    let longest = source.width().max(source.height()).max(1) as f64;
    let width = (source.width() as f64 / longest * edge as f64)
        .round()
        .max(1.0) as u32;
    let height = (source.height() as f64 / longest * edge as f64)
        .round()
        .max(1.0) as u32;
    if source.dimensions() == (width, height) {
        return source;
    }
    // Filtering straight-alpha pixels introduces dark/coloured fringes from
    // transparent pixels. Filter premultiplied channels, then restore RGBA.
    for pixel in source.pixels_mut() {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            *channel = ((u32::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    let mut output = image::imageops::resize(
        &source,
        width,
        height,
        image::imageops::FilterType::Lanczos3,
    );
    for pixel in output.pixels_mut() {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            *channel = (u32::from(*channel) * 255 + alpha / 2)
                .checked_div(alpha)
                .unwrap_or(0)
                .min(255) as u8;
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtering_preserves_colour_at_transparent_edges() {
        let mut source = image::RgbaImage::from_pixel(144, 144, image::Rgba([0, 255, 0, 0]));
        for y in 24..120 {
            for x in 24..120 {
                source.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        let output = resize(source, 28, 1.0);
        let edges: Vec<_> = output
            .pixels()
            .filter(|pixel| pixel[3] > 10 && pixel[3] < 245)
            .collect();
        assert!(!edges.is_empty(), "downscaled edges must be antialiased");
        assert!(
            edges
                .iter()
                .all(|pixel| pixel[0] >= 250 && pixel[1] == 0 && pixel[2] == 0)
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
            let source = image::RgbaImage::new(144, 72);
            assert_eq!(resize(source, 28, scale).dimensions(), expected);
        }
    }
}
