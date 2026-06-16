use crate::image_norm::NormalizedImage;

/// Sobel edge histogram: `orientation_bins` × `grid`×`grid` cells → Vec<u16>.
/// Length = orientation_bins * grid * grid.
pub fn edge_histogram(img: &NormalizedImage, orientation_bins: usize, grid: usize) -> Vec<u16> {
    let size = img.width as usize;
    // Luminance channel
    let lum: Vec<f32> = img
        .pixels_rgb_f32
        .iter()
        .map(|[r, g, b]| 0.299 * r + 0.587 * g + 0.114 * b)
        .collect();

    let cell_w = size / grid;
    let cell_h = size / grid;

    let total_bins = orientation_bins * grid * grid;
    let mut hist = vec![0u32; total_bins];

    for gy in 0..grid {
        for gx in 0..grid {
            let cell_idx = gy * grid + gx;
            let x0 = gx * cell_w;
            let y0 = gy * cell_h;
            let x1 = (x0 + cell_w).min(size);
            let y1 = (y0 + cell_h).min(size);

            for y in y0..y1 {
                for x in x0..x1 {
                    // Sobel kernels (clamp at borders)
                    let gx_val = sobel_x(&lum, x, y, size);
                    let gy_val = sobel_y(&lum, x, y, size);
                    let mag = (gx_val * gx_val + gy_val * gy_val).sqrt();
                    if mag < 1e-6 {
                        continue;
                    }
                    // Angle in [0, pi) — unsigned orientation
                    let angle = gy_val.atan2(gx_val);
                    let angle_norm = (angle + std::f32::consts::PI) / (std::f32::consts::PI);
                    let bin =
                        ((angle_norm * orientation_bins as f32) as usize).min(orientation_bins - 1);
                    hist[cell_idx * orientation_bins + bin] += (mag * 1000.0) as u32;
                }
            }
        }
    }

    hist.into_iter()
        .map(|v| v.min(u16::MAX as u32) as u16)
        .collect()
}

fn pixel(lum: &[f32], x: isize, y: isize, size: usize) -> f32 {
    let xc = x.clamp(0, size as isize - 1) as usize;
    let yc = y.clamp(0, size as isize - 1) as usize;
    lum[yc * size + xc]
}

fn sobel_x(lum: &[f32], x: usize, y: usize, size: usize) -> f32 {
    let (xi, yi) = (x as isize, y as isize);
    -pixel(lum, xi - 1, yi - 1, size)
        - 2.0 * pixel(lum, xi - 1, yi, size)
        - pixel(lum, xi - 1, yi + 1, size)
        + pixel(lum, xi + 1, yi - 1, size)
        + 2.0 * pixel(lum, xi + 1, yi, size)
        + pixel(lum, xi + 1, yi + 1, size)
}

fn sobel_y(lum: &[f32], x: usize, y: usize, size: usize) -> f32 {
    let (xi, yi) = (x as isize, y as isize);
    -pixel(lum, xi - 1, yi - 1, size)
        - 2.0 * pixel(lum, xi, yi - 1, size)
        - pixel(lum, xi + 1, yi - 1, size)
        + pixel(lum, xi - 1, yi + 1, size)
        + 2.0 * pixel(lum, xi, yi + 1, size)
        + pixel(lum, xi + 1, yi + 1, size)
}

/// L1 similarity (higher = more similar) between two edge histograms.
/// Returns 1.0 - normalized_L1_distance.
pub fn edge_similarity(a: &[u16], b: &[u16]) -> f32 {
    assert_eq!(a.len(), b.len());
    let sum_diff: u64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| (x as i32 - y as i32).unsigned_abs() as u64)
        .sum();
    let sum_max: u64 = a.iter().zip(b).map(|(&x, &y)| x.max(y) as u64).sum();
    if sum_max == 0 {
        return 1.0;
    }
    1.0 - (sum_diff as f32 / sum_max as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_norm::NormalizedImage;

    fn make_norm(size: u32) -> NormalizedImage {
        let mut pixels = vec![[0f32; 3]; (size * size) as usize];
        // Create a gradient pattern
        for i in 0..(size * size) as usize {
            let v = (i as f32) / (size * size) as f32;
            pixels[i] = [v, v * 0.5, 0.0];
        }
        NormalizedImage {
            width: size,
            height: size,
            pixels_rgb_f32: pixels,
        }
    }

    #[test]
    fn edge_hist_length() {
        let img = make_norm(128);
        let h = edge_histogram(&img, 8, 4);
        assert_eq!(h.len(), 128); // 8 * 4 * 4
    }

    #[test]
    fn edge_hist_deterministic() {
        let img = make_norm(128);
        let h1 = edge_histogram(&img, 8, 4);
        let h2 = edge_histogram(&img, 8, 4);
        assert_eq!(h1, h2);
    }

    #[test]
    fn edge_self_similarity() {
        let img = make_norm(128);
        let h = edge_histogram(&img, 8, 4);
        assert!((edge_similarity(&h, &h) - 1.0).abs() < 1e-5);
    }
}
