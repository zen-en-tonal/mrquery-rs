mod commands;
mod jsonl;
mod protocol;

use std::path::PathBuf;

use clap::Parser;
use mrq_core::config::MrqConfig;

use commands::WorkerState;

#[derive(Parser)]
#[command(
    name = "mrq-worker",
    about = "mrquery-rs Elixir Port worker (JSONL protocol)"
)]
struct Args {
    #[arg(long)]
    db: PathBuf,
    #[arg(long)]
    config: Option<PathBuf>,
}

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();

    let cfg = match args.config {
        Some(ref p) => {
            let s = std::fs::read_to_string(p).expect("Failed to read config file");
            MrqConfig::from_toml(&s).expect("Failed to parse config")
        }
        None => MrqConfig::default(),
    };

    let mut state = WorkerState::new(args.db, cfg);

    // Attempt initial load; non-fatal if no index yet
    if let Err(e) = state.reload() {
        tracing::warn!("Initial index load failed (may be empty): {e}");
    }

    jsonl::run_loop(&mut state);
}
