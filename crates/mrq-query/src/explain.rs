use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchExplanation {
    pub wavelet_score: f32,
    pub color_distance: f32,
    pub edge_similarity: f32,
    pub hash_distance: u32,
    pub aspect_ratio_penalty: f32,
}
