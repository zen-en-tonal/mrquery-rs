use std::path::Path;
use std::time::Instant;

use mrq_core::{
    config::MrqConfig,
    image_norm::{normalize_from_bytes, normalize_from_path},
    score::{ScoringProfile, ScoringWeights},
    signature::{extract_signature, ImageId, ImageSignature},
    MrqError, Result,
};
use mrq_index::reader::IndexReader;
use serde::{Deserialize, Serialize};

use crate::{
    candidate::generate_candidates,
    rank::{rerank, SearchResult},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryMode {
    Image,
    Sketch,
    Duplicate,
    Region,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ImageSourceQuery {
    Path(String),
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageQuery {
    pub source: ImageSourceQuery,
    pub mode: QueryMode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryOptions {
    pub top_k: usize,
    pub candidate_limit: usize,
    pub scoring_profile: ScoringProfile,
    pub include_explanation: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            top_k: 20,
            candidate_limit: 200,
            scoring_profile: ScoringProfile::Image,
            include_explanation: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryStats {
    pub candidates: usize,
    pub elapsed_ms: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryResponse {
    pub index_version: u64,
    pub results: Vec<SearchResult>,
    pub stats: QueryStats,
}

pub struct QueryEngine {
    pub reader: IndexReader,
    pub cfg: MrqConfig,
}

impl QueryEngine {
    pub fn open(db_path: impl AsRef<Path>, cfg: MrqConfig) -> Result<Self> {
        let reader = IndexReader::open(db_path)?;
        Ok(Self { reader, cfg })
    }

    pub fn query_by_image(&self, query: ImageQuery, opts: QueryOptions) -> Result<QueryResponse> {
        if matches!(query.mode, QueryMode::Region) {
            return Err(MrqError::Unsupported(
                "Region mode not supported in MVP".to_string(),
            ));
        }
        if opts.top_k == 0 {
            return Err(MrqError::InvalidRequest("top_k must be > 0".to_string()));
        }

        let start = Instant::now();

        let norm = match &query.source {
            ImageSourceQuery::Path(p) => normalize_from_path(
                Path::new(p),
                self.cfg.image.size,
                self.cfg.image.max_input_pixels,
                self.cfg.image.background,
            )?,
            ImageSourceQuery::Bytes(b) => normalize_from_bytes(
                b,
                self.cfg.image.size,
                self.cfg.image.max_input_pixels,
                self.cfg.image.background,
            )?,
        };

        let query_sig = extract_signature(0, &norm, &self.cfg);
        self.query_by_signature(query_sig, opts, start)
    }

    pub fn query_by_signature_ext(
        &self,
        signature: ImageSignature,
        opts: QueryOptions,
    ) -> Result<QueryResponse> {
        self.query_by_signature(signature, opts, Instant::now())
    }

    fn query_by_signature(
        &self,
        query_sig: ImageSignature,
        opts: QueryOptions,
        start: Instant,
    ) -> Result<QueryResponse> {
        let profile = opts.scoring_profile;
        let weights = ScoringWeights::for_profile(profile, &self.cfg);

        // Stage 1: candidate generation via inverted index (or linear fallback)
        let candidates = if self.reader.posting_index.map.is_empty() {
            // Linear fallback
            self.linear_candidates(&query_sig, opts.candidate_limit)
        } else {
            generate_candidates(&self.reader.posting_index, &query_sig, opts.candidate_limit)
        };

        let num_candidates = candidates.len();

        // Stage 2: re-rank with color/edge/hash
        let results = rerank(
            &candidates,
            &query_sig,
            &self.reader,
            profile,
            &weights,
            opts.top_k,
            opts.include_explanation,
        );

        let elapsed_ms = start.elapsed().as_secs_f32() * 1000.0;
        Ok(QueryResponse {
            index_version: self.reader.version,
            results,
            stats: QueryStats {
                candidates: num_candidates,
                elapsed_ms,
            },
        })
    }

    /// Linear scan returning (image_id, 1.0) for all non-deleted images.
    fn linear_candidates(&self, query_sig: &ImageSignature, limit: usize) -> Vec<(ImageId, f32)> {
        let mut scored: Vec<(ImageId, f32)> = self
            .reader
            .signatures
            .iter()
            .filter(|s| !self.reader.is_deleted(s.image_id))
            .map(|s| {
                // Quick wavelet overlap count
                let q_set: std::collections::HashSet<_> = query_sig.wavelet_tokens.iter().collect();
                let overlap = s
                    .wavelet_tokens
                    .iter()
                    .filter(|t| q_set.contains(t))
                    .count();
                (s.image_id, overlap as f32)
            })
            .collect();
        scored.sort_unstable_by(|(a_id, a_sc), (b_id, b_sc)| {
            b_sc.partial_cmp(a_sc).unwrap().then_with(|| a_id.cmp(b_id))
        });
        scored.truncate(limit);
        scored
    }
}
