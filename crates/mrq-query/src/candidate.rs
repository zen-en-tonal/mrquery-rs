use std::collections::HashMap;

use mrq_core::signature::{ImageId, ImageSignature};
use mrq_index::postings::WaveletPostingIndex;

/// Accumulate wavelet scores from posting lists, return top `candidate_limit` candidates.
pub fn generate_candidates(
    index: &WaveletPostingIndex,
    query_sig: &ImageSignature,
    candidate_limit: usize,
) -> Vec<(ImageId, f32)> {
    let mut scores: HashMap<ImageId, f32> = HashMap::new();
    for tok in &query_sig.wavelet_tokens {
        for posting in index.lookup(tok) {
            *scores.entry(posting.image_id).or_default() += posting.weight;
        }
    }
    let mut candidates: Vec<(ImageId, f32)> = scores.into_iter().collect();
    // Descending score, then ascending id for determinism
    candidates.sort_unstable_by(|(a_id, a_sc), (b_id, b_sc)| {
        b_sc.partial_cmp(a_sc).unwrap().then_with(|| a_id.cmp(b_id))
    });
    candidates.truncate(candidate_limit);
    candidates
}
