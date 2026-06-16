use serde::{Deserialize, Serialize};

use crate::{
    color::{avg_color, color_histogram},
    config::MrqConfig,
    edge::edge_histogram,
    hash::average_hash,
    image_norm::NormalizedImage,
    wavelet::{extract_tokens, WaveletToken},
};

pub type ImageId = u64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageSignature {
    pub image_id: ImageId,
    pub width: u32,
    pub height: u32,
    pub avg_color: [f32; 3],
    pub wavelet_tokens: Vec<WaveletToken>,
    pub color_hist: Vec<u16>,
    pub edge_hist: Vec<u16>,
    pub phash: u64,
}

pub fn extract_signature(
    image_id: ImageId,
    img: &NormalizedImage,
    cfg: &MrqConfig,
) -> ImageSignature {
    ImageSignature {
        image_id,
        width: img.width,
        height: img.height,
        avg_color: avg_color(img),
        wavelet_tokens: extract_tokens(img, cfg.wavelet.top_k),
        color_hist: color_histogram(img, cfg.color.hist_bins),
        edge_hist: edge_histogram(img, cfg.edge.orientation_bins, cfg.edge.grid),
        phash: average_hash(img),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_img() -> NormalizedImage {
        let mut pixels = vec![[0f32; 3]; 128 * 128];
        for (i, p) in pixels.iter_mut().enumerate() {
            let v = (i as f32) / (128.0 * 128.0);
            *p = [v, 1.0 - v, v * 0.5];
        }
        NormalizedImage {
            width: 128,
            height: 128,
            pixels_rgb_f32: pixels,
        }
    }

    #[test]
    fn signature_deterministic() {
        let img = make_img();
        let cfg = MrqConfig::default();
        let s1 = extract_signature(1, &img, &cfg);
        let s2 = extract_signature(1, &img, &cfg);
        assert_eq!(s1.wavelet_tokens, s2.wavelet_tokens);
        assert_eq!(s1.color_hist, s2.color_hist);
        assert_eq!(s1.edge_hist, s2.edge_hist);
        assert_eq!(s1.phash, s2.phash);
    }
}
