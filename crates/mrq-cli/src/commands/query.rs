use std::path::PathBuf;

use clap::{Args, ValueEnum};
use mrq_core::{config::MrqConfig, score::ScoringProfile};
use mrq_query::engine::{ImageQuery, ImageSourceQuery, QueryEngine, QueryMode, QueryOptions};

#[derive(Clone, ValueEnum)]
pub enum CliMode {
    Image,
    Sketch,
    Duplicate,
}

#[derive(Args)]
pub struct QueryArgs {
    #[arg(long)]
    pub db: PathBuf,
    #[arg(long)]
    pub image: PathBuf,
    #[arg(long, default_value = "image")]
    pub mode: CliMode,
    #[arg(long, default_value = "20")]
    pub top_k: usize,
    #[arg(long, default_value = "200")]
    pub candidate_limit: usize,
    #[arg(long)]
    pub explain: bool,
}

pub fn run(args: QueryArgs) -> anyhow::Result<()> {
    let cfg = MrqConfig::default();
    let engine = QueryEngine::open(&args.db, cfg)?;

    let profile = match args.mode {
        CliMode::Image => ScoringProfile::Image,
        CliMode::Sketch => ScoringProfile::Sketch,
        CliMode::Duplicate => ScoringProfile::Duplicate,
    };

    let query = ImageQuery {
        source: ImageSourceQuery::Path(args.image.to_string_lossy().to_string()),
        mode: match args.mode {
            CliMode::Image => QueryMode::Image,
            CliMode::Sketch => QueryMode::Sketch,
            CliMode::Duplicate => QueryMode::Duplicate,
        },
    };

    let opts = QueryOptions {
        top_k: args.top_k,
        candidate_limit: args.candidate_limit,
        scoring_profile: profile,
        include_explanation: args.explain,
    };

    let resp = engine.query_by_image(query, opts)?;
    println!(
        "Results (index_version={}, candidates={}, elapsed={:.2}ms):",
        resp.index_version, resp.stats.candidates, resp.stats.elapsed_ms
    );
    for r in &resp.results {
        print!(
            "  #{} id={} ext_id={} score={:.4}",
            r.rank, r.image_id, r.external_id, r.score
        );
        if let Some(ex) = &r.explanation {
            print!(
                " [wavelet={:.3} color_dist={:.3} edge_sim={:.3} hamming={} aspect={:.3}]",
                ex.wavelet_score,
                ex.color_distance,
                ex.edge_similarity,
                ex.hash_distance,
                ex.aspect_ratio_penalty
            );
        }
        println!();
    }
    Ok(())
}
