use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use mrq_core::{
    config::MrqConfig,
    image_norm,
    signature::{extract_signature, ImageId, ImageSignature},
    MrqError, Result,
};

use crate::{
    postings::WaveletPostingIndex,
    store::{read_current_version, version_dir, write_current_version, write_version},
    types::{
        ApplyResult, BatchId, DocumentMetadata, ImageDocument, ImageSource, LoadedIndex,
        WriteBatch, WriteOperation,
    },
};

pub struct IndexWriter {
    pub db_path: PathBuf,
    pub cfg: MrqConfig,
    /// Current committed version
    pub current_version: u64,
    /// State carried between apply_batch calls (committed on commit())
    state: IndexState,
}

struct IndexState {
    signatures: HashMap<ImageId, ImageSignature>,
    metadata: HashMap<ImageId, DocumentMetadata>,
    deleted_ids: HashSet<ImageId>,
    applied_batches: Vec<BatchId>,
}

impl IndexWriter {
    pub fn create(db_path: impl AsRef<Path>, cfg: MrqConfig) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        fs::create_dir_all(&db_path)?;
        fs::create_dir_all(db_path.join("versions"))?;
        Ok(Self {
            db_path,
            cfg,
            current_version: 0,
            state: IndexState::empty(),
        })
    }

    pub fn open(db_path: impl AsRef<Path>, cfg: MrqConfig) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let current_version = read_current_version(&db_path)?;
        let state = if current_version == 0 {
            IndexState::empty()
        } else {
            let loaded = crate::store::load_version(&db_path, current_version)?;
            IndexState::from_loaded(loaded)
        };
        Ok(Self {
            db_path,
            cfg,
            current_version,
            state,
        })
    }

    pub fn apply_batch(&mut self, batch: WriteBatch) -> Result<ApplyResult> {
        // Idempotency check
        if self.state.applied_batches.contains(&batch.batch_id) {
            return Ok(ApplyResult {
                batch_id: batch.batch_id,
                previous_version: self.current_version,
                new_version: self.current_version,
                added: 0,
                updated: 0,
                removed: 0,
            });
        }

        // Version check
        if batch.base_index_version != self.current_version {
            return Err(MrqError::VersionMismatch {
                expected: self.current_version,
                got: batch.base_index_version,
            });
        }

        let prev = self.current_version;
        let mut added = 0usize;
        let mut updated = 0usize;
        let mut removed = 0usize;

        for op in batch.operations {
            match op {
                WriteOperation::Add(doc) => {
                    let sig = self.load_signature(&doc)?;
                    let is_update = self.state.signatures.contains_key(&doc.image_id);
                    self.state.signatures.insert(doc.image_id, sig);
                    self.state.metadata.insert(doc.image_id, doc_to_meta(&doc));
                    self.state.deleted_ids.remove(&doc.image_id);
                    if is_update {
                        updated += 1;
                    } else {
                        added += 1;
                    }
                }
                WriteOperation::Update(doc) => {
                    let sig = self.load_signature(&doc)?;
                    let was_present = self.state.signatures.contains_key(&doc.image_id);
                    self.state.signatures.insert(doc.image_id, sig);
                    self.state.metadata.insert(doc.image_id, doc_to_meta(&doc));
                    self.state.deleted_ids.remove(&doc.image_id);
                    if was_present {
                        updated += 1;
                    } else {
                        added += 1;
                    }
                }
                WriteOperation::Remove { image_id } => {
                    self.state.signatures.remove(&image_id);
                    self.state.metadata.remove(&image_id);
                    self.state.deleted_ids.insert(image_id);
                    removed += 1;
                }
            }
        }

        self.state.applied_batches.push(batch.batch_id.clone());
        self.current_version += 1;

        // Commit immediately (full rebuild per apply)
        self.flush()?;

        Ok(ApplyResult {
            batch_id: batch.batch_id,
            previous_version: prev,
            new_version: self.current_version,
            added,
            updated,
            removed,
        })
    }

    fn load_signature(&self, doc: &ImageDocument) -> Result<ImageSignature> {
        let norm = match &doc.source {
            ImageSource::Path(p) => {
                let path = std::path::Path::new(p);
                image_norm::normalize_from_path(
                    path,
                    self.cfg.image.size,
                    self.cfg.image.max_input_pixels,
                    self.cfg.image.background,
                )?
            }
            ImageSource::Bytes(b) => image_norm::normalize_from_bytes(
                b,
                self.cfg.image.size,
                self.cfg.image.max_input_pixels,
                self.cfg.image.background,
            )?,
        };
        Ok(extract_signature(doc.image_id, &norm, &self.cfg))
    }

    fn flush(&self) -> Result<()> {
        let sigs: Vec<ImageSignature> = self.state.signatures.values().cloned().collect();
        let meta: Vec<DocumentMetadata> = self.state.metadata.values().cloned().collect();

        // Build and write posting index
        let posting_idx = WaveletPostingIndex::build(&sigs);
        let posting_bytes = posting_idx.to_bytes()?;
        let vdir = version_dir(&self.db_path, self.current_version);
        fs::create_dir_all(&vdir)?;
        let posting_path = vdir.join("wavelet_postings.bin");
        fs::write(&posting_path, &posting_bytes)?;

        // Write everything else
        let mut manifest = write_version(
            &self.db_path,
            self.current_version,
            &sigs,
            &meta,
            &self.state.deleted_ids,
            &self.state.applied_batches,
            self.cfg.image.size,
            self.cfg.wavelet.top_k,
        )?;

        // Add posting index to manifest files
        let posting_size = fs::metadata(&posting_path)?.len();
        let posting_checksum = format!("sha256:{}", crate::store::sha256_file(&posting_path)?);
        manifest.files.push(crate::types::ManifestFile {
            path: "wavelet_postings.bin".to_string(),
            size_bytes: posting_size,
            checksum: posting_checksum,
        });

        // Rewrite manifest with postings entry
        let manifest_path = vdir.join("manifest.json");
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| MrqError::Serialize(e.to_string()))?;
        fs::write(&manifest_path, manifest_json.as_bytes())?;

        write_current_version(&self.db_path, self.current_version)?;
        Ok(())
    }
}

impl IndexState {
    fn empty() -> Self {
        Self {
            signatures: HashMap::new(),
            metadata: HashMap::new(),
            deleted_ids: HashSet::new(),
            applied_batches: Vec::new(),
        }
    }

    fn from_loaded(loaded: LoadedIndex) -> Self {
        let signatures = loaded
            .signatures
            .into_iter()
            .map(|s| (s.image_id, s))
            .collect();
        let metadata = loaded
            .metadata
            .into_iter()
            .map(|m| (m.image_id, m))
            .collect();
        Self {
            signatures,
            metadata,
            deleted_ids: loaded.deleted_ids,
            applied_batches: loaded.manifest.applied_batches,
        }
    }
}

fn doc_to_meta(doc: &ImageDocument) -> DocumentMetadata {
    let path = match &doc.source {
        ImageSource::Path(p) => Some(p.clone()),
        ImageSource::Bytes(_) => None,
    };
    DocumentMetadata {
        image_id: doc.image_id,
        external_id: doc.external_id.clone(),
        original_path: doc.metadata.original_path.clone().or(path),
        width: doc.metadata.width,
        height: doc.metadata.height,
        content_type: doc.metadata.content_type.clone(),
        user_metadata_json: doc.metadata.user_metadata_json.clone(),
    }
}
