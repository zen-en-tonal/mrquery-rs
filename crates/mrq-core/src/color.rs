use crate::image_norm::NormalizedImage;

/// avg_color: mean [R, G, B] across all pixels.
pub fn avg_color(img: &NormalizedImage) -> [f32; 3] {
    let n = img.pixels_rgb_f32.len() as f32;
    let mut sum = [0f32; 3];
    for p in &img.pixels_rgb_f32 {
        sum[0] += p[0];
        sum[1] += p[1];
        sum[2] += p[2];
    }
    [sum[0] / n, sum[1] / n, sum[2] / n]
}

/// RGB histogram with `bins` bins per channel → flat Vec<u16> length = 3 * bins.
pub fn color_histogram(img: &NormalizedImage, bins: usize) -> Vec<u16> {
    let mut hist = vec![0u32; 3 * bins];
    let scale = bins as f32;
    for p in &img.pixels_rgb_f32 {
        for ch in 0..3 {
            let bin = ((p[ch] * scale) as usize).min(bins - 1);
            hist[ch * bins + bin] += 1;
        }
    }
    // Saturating cast to u16
    hist.into_iter()
        .map(|v| v.min(u16::MAX as u32) as u16)
        .collect()
}

/// L2 distance between avg colors.
pub fn avg_color_dist(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let d0 = a[0] - b[0];
    let d1 = a[1] - b[1];
    let d2 = a[2] - b[2];
    (d0 * d0 + d1 * d1 + d2 * d2).sqrt()
}

/// L1 distance between color histograms (normalized by pixel count).
pub fn histogram_dist(a: &[u16], b: &[u16]) -> f32 {
    assert_eq!(a.len(), b.len());
    let sum: u32 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| (x as i32 - y as i32).unsigned_abs())
        .sum();
    sum as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_norm::NormalizedImage;

    fn solid(r: f32, g: f32, b: f32) -> NormalizedImage {
        NormalizedImage {
            width: 4,
            height: 4,
            pixels_rgb_f32: vec![[r, g, b]; 16],
        }
    }

    #[test]
    fn avg_color_solid() {
        let img = solid(0.5, 0.25, 0.75);
        let avg = avg_color(&img);
        assert!((avg[0] - 0.5).abs() < 1e-5);
        assert!((avg[1] - 0.25).abs() < 1e-5);
        assert!((avg[2] - 0.75).abs() < 1e-5);
    }

    #[test]
    fn histogram_length() {
        let img = solid(0.1, 0.5, 0.9);
        let h = color_histogram(&img, 8);
        assert_eq!(h.len(), 24);
    }

    #[test]
    fn same_image_zero_dist() {
        let img = solid(0.3, 0.6, 0.1);
        let h = color_histogram(&img, 8);
        assert_eq!(histogram_dist(&h, &h), 0.0);
    }
}
