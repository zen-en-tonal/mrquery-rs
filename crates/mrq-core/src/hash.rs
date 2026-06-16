use crate::image_norm::NormalizedImage;

/// Average perceptual hash: grayscale → resize 8x8 → compare each pixel to mean → u64.
pub fn average_hash(img: &NormalizedImage) -> u64 {
    let size = 8usize;
    // Downsample to 8x8 using box filter from normalized image
    let src_size = img.width as usize;
    let mut small = vec![0f32; size * size];
    let step = src_size / size;
    for gy in 0..size {
        for gx in 0..size {
            let mut sum = 0f32;
            let mut count = 0u32;
            for dy in 0..step {
                for dx in 0..step {
                    let sy = gy * step + dy;
                    let sx = gx * step + dx;
                    if sy < src_size && sx < src_size {
                        let p = &img.pixels_rgb_f32[sy * src_size + sx];
                        // Luminance
                        sum += 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2];
                        count += 1;
                    }
                }
            }
            small[gy * size + gx] = if count > 0 { sum / count as f32 } else { 0.0 };
        }
    }

    let mean = small.iter().sum::<f32>() / 64.0;
    let mut hash = 0u64;
    for (i, &v) in small.iter().enumerate() {
        if v > mean {
            hash |= 1u64 << i;
        }
    }
    hash
}

pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_norm::NormalizedImage;

    fn solid(v: f32) -> NormalizedImage {
        NormalizedImage {
            width: 128,
            height: 128,
            pixels_rgb_f32: vec![[v, v, v]; 128 * 128],
        }
    }

    #[test]
    fn hash_deterministic() {
        let img = solid(0.5);
        assert_eq!(average_hash(&img), average_hash(&img));
    }

    #[test]
    fn hamming_self_zero() {
        let h = average_hash(&solid(0.3));
        assert_eq!(hamming_distance(h, h), 0);
    }

    #[test]
    fn hamming_range() {
        let a = average_hash(&solid(0.1));
        let b = average_hash(&solid(0.9));
        assert!(hamming_distance(a, b) <= 64);
    }
}
