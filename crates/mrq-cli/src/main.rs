mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mrq", about = "mrquery-rs image search CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Index(commands::index::IndexArgs),
    Query(commands::query::QueryArgs),
    Inspect(commands::inspect::InspectArgs),
    Snapshot(commands::snapshot::SnapshotCmd),
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Index(args) => commands::index::run(args),
        Commands::Query(args) => commands::query::run(args),
        Commands::Inspect(args) => commands::inspect::run(args),
        Commands::Snapshot(cmd) => commands::snapshot::run(cmd),
    }
}
