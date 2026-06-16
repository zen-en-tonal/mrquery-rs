use std::path::{Path, PathBuf};

use mrq_core::{config::MrqConfig, score::ScoringProfile};
use mrq_index::{snapshot::create_snapshot, types::WriteBatch, writer::IndexWriter};
use mrq_query::engine::{ImageQuery, ImageSourceQuery, QueryEngine, QueryMode, QueryOptions};
use serde::Deserialize;
use serde_json::Value;

pub struct WorkerState {
    pub db_path: PathBuf,
    pub cfg: MrqConfig,
    pub engine: Option<QueryEngine>,
}

impl WorkerState {
    pub fn new(db_path: PathBuf, cfg: MrqConfig) -> Self {
        Self {
            db_path,
            cfg,
            engine: None,
        }
    }

    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.engine = Some(QueryEngine::open(&self.db_path, self.cfg.clone())?);
        Ok(())
    }

    pub fn engine(&self) -> anyhow::Result<&QueryEngine> {
        self.engine
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No index loaded — try reload command"))
    }
}

pub fn handle_query(state: &WorkerState, payload: &Value) -> anyhow::Result<Value> {
    #[derive(Deserialize)]
    struct QueryPayload {
        image_path: String,
        #[serde(default = "default_mode")]
        mode: String,
        #[serde(default = "default_top_k")]
        top_k: usize,
        #[serde(default = "default_candidate_limit")]
        candidate_limit: usize,
        #[serde(default)]
        include_explanation: bool,
    }
    fn default_mode() -> String {
        "image".to_string()
    }
    fn default_top_k() -> usize {
        20
    }
    fn default_candidate_limit() -> usize {
        200
    }

    let p: QueryPayload = serde_json::from_value(payload.clone())?;
    let profile = match p.mode.as_str() {
        "sketch" => ScoringProfile::Sketch,
        "duplicate" => ScoringProfile::Duplicate,
        _ => ScoringProfile::Image,
    };
    let query_mode = match p.mode.as_str() {
        "sketch" => QueryMode::Sketch,
        "duplicate" => QueryMode::Duplicate,
        _ => QueryMode::Image,
    };

    let query = ImageQuery {
        source: ImageSourceQuery::Path(p.image_path),
        mode: query_mode,
    };
    let opts = QueryOptions {
        top_k: p.top_k,
        candidate_limit: p.candidate_limit,
        scoring_profile: profile,
        include_explanation: p.include_explanation,
    };

    let resp = state.engine()?.query_by_image(query, opts)?;
    Ok(serde_json::to_value(resp)?)
}

pub fn handle_apply_batch(state: &mut WorkerState, payload: &Value) -> anyhow::Result<Value> {
    let batch: WriteBatch = serde_json::from_value(payload.clone())?;
    let mut writer = IndexWriter::open(&state.db_path, state.cfg.clone())?;
    let result = writer.apply_batch(batch)?;
    // Reload engine after write
    state.reload().ok();
    Ok(serde_json::to_value(result)?)
}

pub fn handle_create_snapshot(state: &WorkerState, payload: &Value) -> anyhow::Result<Value> {
    #[derive(Deserialize)]
    struct SnapshotPayload {
        output_path: String,
    }
    let p: SnapshotPayload = serde_json::from_value(payload.clone())?;
    let manifest = create_snapshot(&state.db_path, Path::new(&p.output_path))?;
    Ok(serde_json::json!({
        "index_version": manifest.index_version,
        "files": manifest.files.len(),
    }))
}

pub fn handle_reload(state: &mut WorkerState) -> anyhow::Result<Value> {
    state.reload()?;
    let v = state.engine().map(|e| e.reader.version).unwrap_or(0);
    Ok(serde_json::json!({ "index_version": v }))
}
