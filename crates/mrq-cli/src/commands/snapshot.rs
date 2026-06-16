use std::path::PathBuf;

use clap::{Args, Subcommand};
use mrq_index::snapshot::{create_snapshot, load_snapshot};

#[derive(Args)]
pub struct SnapshotCmd {
    #[command(subcommand)]
    pub action: SnapshotAction,
}

#[derive(Subcommand)]
pub enum SnapshotAction {
    Create(SnapshotCreateArgs),
    Load(SnapshotLoadArgs),
}

#[derive(Args)]
pub struct SnapshotCreateArgs {
    #[arg(long)]
    pub db: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Args)]
pub struct SnapshotLoadArgs {
    #[arg(long)]
    pub snapshot: PathBuf,
    #[arg(long)]
    pub db: PathBuf,
}

pub fn run(cmd: SnapshotCmd) -> anyhow::Result<()> {
    match cmd.action {
        SnapshotAction::Create(args) => {
            let manifest = create_snapshot(&args.db, &args.output)?;
            println!(
                "Snapshot created: version={} files={}",
                manifest.index_version,
                manifest.files.len()
            );
        }
        SnapshotAction::Load(args) => {
            let version = load_snapshot(&args.snapshot, &args.db)?;
            println!("Snapshot loaded: version={}", version);
        }
    }
    Ok(())
}
