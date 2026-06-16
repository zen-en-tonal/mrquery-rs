use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use mrq_core::{
    signature::{ImageId, ImageSignature},
    MrqError, Result,
};

use crate::{
    postings::WaveletPostingIndex,
    store::{load_version, read_current_version, version_dir},
    types::{DocumentMetadata, IndexVersion, VersionManifest},
};

pub struct IndexReader {
    pub db_path: PathBuf,
    pub version: IndexVersion,
    pub signatures: Vec<ImageSignature>,
    pub metadata: Vec<DocumentMetadata>,
    pub deleted_ids: HashSet<ImageId>,
    pub manifest: VersionManifest,
    pub posting_index: WaveletPostingIndex,
}

impl IndexReader {
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let version = read_current_version(&db_path)?;
        if version == 0 {
            return Err(MrqError::NotFound(
                "No committed index version found".to_string(),
            ));
        }
        Self::open_version(&db_path, version)
    }

    pub fn open_version(db_path: &Path, version: IndexVersion) -> Result<Self> {
        let loaded = load_version(db_path, version)?;
        let vdir = version_dir(db_path, version);
        let posting_path = vdir.join("wavelet_postings.bin");
        let posting_index = if posting_path.exists() {
            let bytes = fs::read(&posting_path)?;
            WaveletPostingIndex::from_bytes(&bytes)?
        } else {
            WaveletPostingIndex::default()
        };
        Ok(Self {
            db_path: db_path.to_path_buf(),
            version: loaded.version,
            signatures: loaded.signatures,
            metadata: loaded.metadata,
            deleted_ids: loaded.deleted_ids,
            manifest: loaded.manifest,
            posting_index,
        })
    }

    pub fn signature_by_id(&self, id: ImageId) -> Option<&ImageSignature> {
        self.signatures.iter().find(|s| s.image_id == id)
    }

    pub fn is_deleted(&self, id: ImageId) -> bool {
        self.deleted_ids.contains(&id)
    }
}
