use std::path::PathBuf;

use clap::Args;
use mrq_index::reader::IndexReader;

#[derive(Args)]
pub struct InspectArgs {
    #[arg(long)]
    pub db: PathBuf,
    #[arg(long)]
    pub image_id: u64,
}

pub fn run(args: InspectArgs) -> anyhow::Result<()> {
    let reader = IndexReader::open(&args.db)?;
    match reader.signature_by_id(args.image_id) {
        Some(sig) => {
            println!("image_id: {}", sig.image_id);
            println!("size: {}x{}", sig.width, sig.height);
            println!("avg_color: {:?}", sig.avg_color);
            println!("wavelet_tokens: {} tokens", sig.wavelet_tokens.len());
            println!("color_hist len: {}", sig.color_hist.len());
            println!("edge_hist len: {}", sig.edge_hist.len());
            println!("phash: {:016x}", sig.phash);
        }
        None => {
            println!(
                "image_id {} not found in index v{}",
                args.image_id, reader.version
            );
        }
    }
    Ok(())
}
