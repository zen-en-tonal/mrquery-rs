use serde::{Deserialize, Serialize};

use crate::image_norm::NormalizedImage;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct WaveletToken {
    pub channel: u8,
    pub scale: u8,
    pub band: u8,
    pub x: u16,
    pub y: u16,
    pub sign: i8,
}

/// Run 2D Haar transform on a square power-of-2 buffer in-place.
/// `size` must be a power of 2.
fn haar_2d(buf: &mut Vec<f32>, size: usize) {
    let mut half = size / 2;
    while half >= 1 {
        let full = half * 2;
        // horizontal pass over rows
        for row in 0..(size) {
            let base = row * size;
            let mut tmp = vec![0f32; full];
            for i in 0..half {
                let a = buf[base + 2 * i];
                let b = buf[base + 2 * i + 1];
                tmp[i] = (a + b) * 0.5;
                tmp[half + i] = (a - b) * 0.5;
            }
            buf[base..base + full].copy_from_slice(&tmp);
        }
        // vertical pass over columns
        for col in 0..size {
            let mut tmp = vec![0f32; full];
            for i in 0..half {
                let a = buf[(2 * i) * size + col];
                let b = buf[(2 * i + 1) * size + col];
                tmp[i] = (a + b) * 0.5;
                tmp[half + i] = (a - b) * 0.5;
            }
            for i in 0..full {
                buf[i * size + col] = tmp[i];
            }
        }
        half /= 2;
    }
}

/// Extract top-k WaveletTokens from a NormalizedImage.
/// Deterministic: tie-break by (channel asc, scale asc, band asc, x asc, y asc).
pub fn extract_tokens(img: &NormalizedImage, top_k: usize) -> Vec<WaveletToken> {
    let size = img.width as usize;
    assert_eq!(img.width, img.height);

    // Must be power of 2 for Haar
    let log2_size = (size as f64).log2() as u32;
    assert_eq!(1 << log2_size, size, "image size must be power of 2");

    let mut all: Vec<(f32, WaveletToken)> = Vec::new();

    for ch in 0u8..3 {
        let mut buf: Vec<f32> = img.pixels_rgb_f32.iter().map(|p| p[ch as usize]).collect();
        haar_2d(&mut buf, size);

        // Enumerate coefficients with their subband info
        let mut level_size = size;
        let mut scale = 0u8;
        while level_size > 1 {
            let half = level_size / 2;
            // 3 subbands per level: LH (band=0), HL (band=1), HH (band=2)
            // LH: rows [0,half), cols [half,level_size)
            for band in 0u8..3 {
                let (row_start, row_end, col_start, col_end) = match band {
                    0 => (0, half, half, level_size),          // LH
                    1 => (half, level_size, 0, half),          // HL
                    2 => (half, level_size, half, level_size), // HH
                    _ => unreachable!(),
                };
                for row in row_start..row_end {
                    for col in col_start..col_end {
                        let val = buf[row * size + col];
                        if val == 0.0 {
                            continue;
                        }
                        let sign = if val > 0.0 { 1i8 } else { -1i8 };
                        all.push((
                            val.abs(),
                            WaveletToken {
                                channel: ch,
                                scale,
                                band,
                                x: col as u16,
                                y: row as u16,
                                sign,
                            },
                        ));
                    }
                }
            }
            level_size /= 2;
            scale += 1;
        }
    }

    // Sort: abs desc, then token asc for determinism
    all.sort_unstable_by(|(a_abs, a_tok), (b_abs, b_tok)| {
        b_abs
            .partial_cmp(a_abs)
            .unwrap()
            .then_with(|| a_tok.cmp(b_tok))
    });

    all.truncate(top_k);
    all.into_iter().map(|(_, tok)| tok).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_norm::NormalizedImage;

    fn make_norm(size: u32, val: f32) -> NormalizedImage {
        NormalizedImage {
            width: size,
            height: size,
            pixels_rgb_f32: vec![[val, val * 0.5, val * 0.25]; (size * size) as usize],
        }
    }

    #[test]
    fn tokens_deterministic() {
        let img = make_norm(128, 0.5);
        let t1 = extract_tokens(&img, 64);
        let t2 = extract_tokens(&img, 64);
        assert_eq!(t1, t2);
    }

    #[test]
    fn tokens_no_duplicates() {
        let img = make_norm(128, 0.7);
        let tokens = extract_tokens(&img, 64);
        let mut seen = std::collections::HashSet::new();
        for t in &tokens {
            assert!(seen.insert(*t), "duplicate token: {:?}", t);
        }
    }
}
