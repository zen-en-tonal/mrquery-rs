use serde::{Deserialize, Serialize};

use crate::config::MrqConfig;
pub use crate::config::ScoringWeights;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScoringProfile {
    Image,
    Sketch,
    Duplicate,
}

impl ScoringWeights {
    pub fn for_profile(profile: ScoringProfile, cfg: &MrqConfig) -> Self {
        match profile {
            ScoringProfile::Image => cfg.scoring.image.clone(),
            ScoringProfile::Sketch => cfg.scoring.sketch.clone(),
            ScoringProfile::Duplicate => cfg.scoring.duplicate.clone(),
        }
    }
}

pub fn compute_score(
    wavelet_acc: f32,
    color_dist: f32,
    edge_sim: f32,
    hamming: u32,
    aspect_penalty: f32,
    w: &ScoringWeights,
) -> f32 {
    w.wavelet * wavelet_acc - w.color * color_dist + w.edge * edge_sim
        - w.hash * (hamming as f32 / 64.0)
        - w.aspect * aspect_penalty
}
