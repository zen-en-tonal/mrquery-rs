use std::collections::HashMap;

use mrq_core::wavelet::WaveletToken;
use mrq_core::{
    signature::{ImageId, ImageSignature},
    MrqError, Result,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Posting {
    pub image_id: ImageId,
    pub weight: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WaveletPostingIndex {
    pub map: HashMap<WaveletToken, Vec<Posting>>,
}

impl WaveletPostingIndex {
    pub fn build(sigs: &[ImageSignature]) -> Self {
        let mut map: HashMap<WaveletToken, Vec<Posting>> = HashMap::new();
        for sig in sigs {
            // Compute max abs coefficient count for normalization
            let n = sig.wavelet_tokens.len() as f32;
            for (i, tok) in sig.wavelet_tokens.iter().enumerate() {
                // Weight decays by rank position (highest-ranked = most important)
                let weight = 1.0 - (i as f32 / n.max(1.0));
                map.entry(*tok).or_default().push(Posting {
                    image_id: sig.image_id,
                    weight,
                });
            }
        }
        Self { map }
    }

    pub fn lookup(&self, token: &WaveletToken) -> &[Posting] {
        self.map.get(token).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| MrqError::Serialize(e.to_string()))
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        bincode::deserialize(b).map_err(|e| MrqError::Serialize(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrq_core::{config::MrqConfig, image_norm::NormalizedImage, signature::extract_signature};

    fn make_sig(id: ImageId) -> ImageSignature {
        let mut pixels = vec![[0f32; 3]; 128 * 128];
        for (i, p) in pixels.iter_mut().enumerate() {
            let v = ((i as f32 + id as f32 * 13.0) / (128.0 * 128.0)).fract();
            *p = [v, 1.0 - v, v * 0.5];
        }
        let img = NormalizedImage {
            width: 128,
            height: 128,
            pixels_rgb_f32: pixels,
        };
        extract_signature(id, &img, &MrqConfig::default())
    }

    #[test]
    fn build_and_lookup() {
        let sigs: Vec<_> = (1..=5).map(make_sig).collect();
        let idx = WaveletPostingIndex::build(&sigs);
        assert!(!idx.map.is_empty());
    }

    #[test]
    fn roundtrip_bytes() {
        let sigs: Vec<_> = (1..=3).map(make_sig).collect();
        let idx = WaveletPostingIndex::build(&sigs);
        let bytes = idx.to_bytes().unwrap();
        let idx2 = WaveletPostingIndex::from_bytes(&bytes).unwrap();
        assert_eq!(idx.map.len(), idx2.map.len());
    }
}
