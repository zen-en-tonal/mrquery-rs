use std::path::PathBuf;

use clap::Args;
use mrq_core::config::MrqConfig;
use mrq_index::{
    types::{DocumentMetadata, ImageDocument, ImageSource, WriteBatch, WriteOperation},
    writer::IndexWriter,
};
use walkdir::WalkDir;

#[derive(Args)]
pub struct IndexArgs {
    #[arg(long)]
    pub input: PathBuf,
    #[arg(long)]
    pub db: PathBuf,
    #[arg(long, default_value = "128")]
    pub size: u32,
    #[arg(long, default_value = "64")]
    pub wavelet_k: usize,
}

pub fn run(args: IndexArgs) -> anyhow::Result<()> {
    let mut cfg = MrqConfig::default();
    cfg.image.size = args.size;
    cfg.wavelet.top_k = args.wavelet_k;

    let mut writer = if args.db.exists() {
        IndexWriter::open(&args.db, cfg.clone())?
    } else {
        IndexWriter::create(&args.db, cfg.clone())?
    };

    let extensions = ["png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "tif"];

    let mut ops: Vec<WriteOperation> = Vec::new();
    let mut next_id: u64 = 1;

    for entry in WalkDir::new(&args.input)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if !extensions.contains(&ext.as_str()) {
            continue;
        }

        let image_id = next_id;
        next_id += 1;
        let path_str = path.to_string_lossy().to_string();
        let external_id = path.file_name().unwrap().to_string_lossy().to_string();

        ops.push(WriteOperation::Add(ImageDocument {
            image_id,
            external_id: external_id.clone(),
            source: ImageSource::Path(path_str.clone()),
            metadata: DocumentMetadata {
                image_id,
                external_id,
                original_path: Some(path_str),
                width: None,
                height: None,
                content_type: None,
                user_metadata_json: None,
            },
        }));
    }

    if ops.is_empty() {
        println!("No images found in {:?}", args.input);
        return Ok(());
    }

    let count = ops.len();
    let batch = WriteBatch {
        batch_id: format!("cli-index-{}", chrono_now()),
        base_index_version: writer.current_version,
        operations: ops,
    };

    let result = writer.apply_batch(batch)?;
    println!(
        "Indexed {} images → version {} (added={}, updated={}, removed={})",
        count, result.new_version, result.added, result.updated, result.removed
    );
    Ok(())
}

fn chrono_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
