use mrq_core::signature::ImageId;

/// Recall@k: fraction of linear_results[:k] that appear in inverted_results[:k].
pub fn recall_at_k(linear_results: &[ImageId], inverted_results: &[ImageId], k: usize) -> f32 {
    let relevant: std::collections::HashSet<_> = linear_results.iter().take(k).collect();
    if relevant.is_empty() {
        return 1.0;
    }
    let hits = inverted_results
        .iter()
        .take(k)
        .filter(|id| relevant.contains(id))
        .count();
    hits as f32 / relevant.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_recall() {
        let ids = vec![1, 2, 3, 4, 5];
        assert!((recall_at_k(&ids, &ids, 5) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn zero_recall() {
        let linear = vec![1, 2, 3];
        let inverted = vec![4, 5, 6];
        assert!((recall_at_k(&linear, &inverted, 3)).abs() < 1e-5);
    }

    #[test]
    fn partial_recall() {
        let linear = vec![1, 2, 3, 4];
        let inverted = vec![1, 2, 5, 6];
        let r = recall_at_k(&linear, &inverted, 4);
        assert!((r - 0.5).abs() < 1e-5);
    }
}
