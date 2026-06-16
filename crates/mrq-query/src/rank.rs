use mrq_core::{
    color::{avg_color_dist, histogram_dist},
    edge::edge_similarity,
    hash::hamming_distance,
    score::{compute_score, ScoringProfile, ScoringWeights},
    signature::ImageId,
};
use mrq_index::reader::IndexReader;
use serde::{Deserialize, Serialize};

use crate::explain::SearchExplanation;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub image_id: ImageId,
    pub external_id: String,
    pub score: f32,
    pub rank: usize,
    pub explanation: Option<SearchExplanation>,
}

pub fn rerank(
    candidates: &[(ImageId, f32)],
    query_sig: &mrq_core::signature::ImageSignature,
    reader: &IndexReader,
    _profile: ScoringProfile,
    weights: &ScoringWeights,
    top_k: usize,
    include_explanation: bool,
) -> Vec<SearchResult> {
    let mut results: Vec<SearchResult> = Vec::with_capacity(candidates.len());

    for &(id, wavelet_score) in candidates {
        if reader.is_deleted(id) {
            continue;
        }
        let Some(sig) = reader.signature_by_id(id) else {
            continue;
        };

        let color_dist = avg_color_dist(&query_sig.avg_color, &sig.avg_color)
            + histogram_dist(&query_sig.color_hist, &sig.color_hist) / 1000.0;
        let edge_sim = edge_similarity(&query_sig.edge_hist, &sig.edge_hist);
        let hamming = hamming_distance(query_sig.phash, sig.phash);
        let aspect_penalty =
            aspect_ratio_penalty(query_sig.width, query_sig.height, sig.width, sig.height);

        let score = compute_score(
            wavelet_score,
            color_dist,
            edge_sim,
            hamming,
            aspect_penalty,
            weights,
        );

        let meta = reader.metadata.iter().find(|m| m.image_id == id);
        let external_id = meta.map(|m| m.external_id.clone()).unwrap_or_default();

        let explanation = if include_explanation {
            Some(SearchExplanation {
                wavelet_score,
                color_distance: color_dist,
                edge_similarity: edge_sim,
                hash_distance: hamming,
                aspect_ratio_penalty: aspect_penalty,
            })
        } else {
            None
        };

        results.push(SearchResult {
            image_id: id,
            external_id,
            score,
            rank: 0,
            explanation,
        });
    }

    // Sort descending score, ascending id tie-break
    results.sort_unstable_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap()
            .then_with(|| a.image_id.cmp(&b.image_id))
    });
    results.truncate(top_k);
    for (i, r) in results.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    results
}

fn aspect_ratio_penalty(qw: u32, qh: u32, tw: u32, th: u32) -> f32 {
    if qh == 0 || th == 0 {
        return 0.0;
    }
    let q_ratio = qw as f32 / qh as f32;
    let t_ratio = tw as f32 / th as f32;
    (q_ratio - t_ratio).abs() / q_ratio.max(t_ratio)
}
