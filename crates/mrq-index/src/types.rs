use mrq_core::signature::{ImageId, ImageSignature};
use serde::{Deserialize, Serialize};

pub type IndexVersion = u64;
pub type BatchId = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub image_id: ImageId,
    pub external_id: String,
    pub original_path: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub content_type: Option<String>,
    pub user_metadata_json: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ImageSource {
    Path(String),
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageDocument {
    pub image_id: ImageId,
    pub external_id: String,
    pub source: ImageSource,
    pub metadata: DocumentMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WriteOperation {
    Add(ImageDocument),
    Remove { image_id: ImageId },
    Update(ImageDocument),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WriteBatch {
    pub batch_id: BatchId,
    pub base_index_version: IndexVersion,
    pub operations: Vec<WriteOperation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplyResult {
    pub batch_id: BatchId,
    pub previous_version: IndexVersion,
    pub new_version: IndexVersion,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub size_bytes: u64,
    pub checksum: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionManifest {
    pub format_version: u32,
    pub index_version: IndexVersion,
    pub created_at_unix_ms: u64,
    pub image_size: u32,
    pub wavelet_top_k: usize,
    pub files: Vec<ManifestFile>,
    pub applied_batches: Vec<BatchId>,
}

/// In-memory loaded index data.
pub struct LoadedIndex {
    pub version: IndexVersion,
    pub signatures: Vec<ImageSignature>,
    pub metadata: Vec<DocumentMetadata>,
    pub deleted_ids: std::collections::HashSet<ImageId>,
    pub manifest: VersionManifest,
}
