use image::{DynamicImage, GenericImageView, ImageReader, Rgb, RgbImage};

use crate::{MrqError, Result};

pub struct NormalizedImage {
    pub width: u32,
    pub height: u32,
    pub pixels_rgb_f32: Vec<[f32; 3]>,
}

pub fn normalize_from_path(
    path: &std::path::Path,
    size: u32,
    max_pixels: u64,
    bg: [f32; 3],
) -> Result<NormalizedImage> {
    let img = ImageReader::open(path)
        .map_err(|e| MrqError::Io(e.into()))?
        .decode()
        .map_err(|e| MrqError::Decode(e.to_string()))?;
    normalize_image(img, size, max_pixels, bg)
}

pub fn normalize_from_bytes(
    data: &[u8],
    size: u32,
    max_pixels: u64,
    bg: [f32; 3],
) -> Result<NormalizedImage> {
    let img = image::load_from_memory(data).map_err(|e| MrqError::Decode(e.to_string()))?;
    normalize_image(img, size, max_pixels, bg)
}

fn normalize_image(
    img: DynamicImage,
    size: u32,
    max_pixels: u64,
    bg: [f32; 3],
) -> Result<NormalizedImage> {
    let (w, h) = img.dimensions();
    if (w as u64) * (h as u64) > max_pixels {
        return Err(MrqError::InvalidRequest(format!(
            "Image too large: {}x{} exceeds {} pixels",
            w, h, max_pixels
        )));
    }
    let rgb = img.into_rgb8();
    let canvas = letterbox_into_square(&rgb, size, bg);
    let pixels_rgb_f32 = canvas
        .pixels()
        .map(|Rgb([r, g, b])| [*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0])
        .collect();
    Ok(NormalizedImage {
        width: size,
        height: size,
        pixels_rgb_f32,
    })
}

fn letterbox_into_square(src: &RgbImage, size: u32, bg: [f32; 3]) -> RgbImage {
    let (sw, sh) = src.dimensions();
    let scale = (size as f32 / sw.max(sh) as f32).min(1.0);
    let new_w = ((sw as f32 * scale) as u32).max(1);
    let new_h = ((sh as f32 * scale) as u32).max(1);

    let resized = image::imageops::resize(src, new_w, new_h, image::imageops::FilterType::Lanczos3);

    let bg_pixel = Rgb([
        (bg[0] * 255.0) as u8,
        (bg[1] * 255.0) as u8,
        (bg[2] * 255.0) as u8,
    ]);
    let mut canvas = RgbImage::from_pixel(size, size, bg_pixel);

    let x_off = (size - new_w) / 2;
    let y_off = (size - new_h) / 2;
    image::imageops::overlay(&mut canvas, &resized, x_off as i64, y_off as i64);
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_produces_correct_size() {
        // Create a small test image in memory
        let mut img = RgbImage::new(64, 32);
        for px in img.pixels_mut() {
            *px = Rgb([100, 150, 200]);
        }
        let dyn_img = DynamicImage::ImageRgb8(img);
        let norm = normalize_image(dyn_img, 128, 40_000_000, [1.0, 1.0, 1.0]).unwrap();
        assert_eq!(norm.width, 128);
        assert_eq!(norm.height, 128);
        assert_eq!(norm.pixels_rgb_f32.len(), 128 * 128);
    }

    #[test]
    fn normalize_deterministic() {
        let mut img1 = RgbImage::new(100, 100);
        let mut img2 = RgbImage::new(100, 100);
        for (p1, p2) in img1.pixels_mut().zip(img2.pixels_mut()) {
            *p1 = Rgb([42, 84, 168]);
            *p2 = Rgb([42, 84, 168]);
        }
        let n1 = normalize_image(
            DynamicImage::ImageRgb8(img1),
            128,
            40_000_000,
            [1.0, 1.0, 1.0],
        )
        .unwrap();
        let n2 = normalize_image(
            DynamicImage::ImageRgb8(img2),
            128,
            40_000_000,
            [1.0, 1.0, 1.0],
        )
        .unwrap();
        assert_eq!(n1.pixels_rgb_f32, n2.pixels_rgb_f32);
    }
}
