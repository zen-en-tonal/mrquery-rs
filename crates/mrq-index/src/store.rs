use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use mrq_core::{
    signature::{ImageId, ImageSignature},
    MrqError, Result,
};
use sha2::{Digest, Sha256};

use crate::types::{DocumentMetadata, LoadedIndex, ManifestFile, VersionManifest};

const CURRENT_FILE: &str = "CURRENT";
const VERSIONS_DIR: &str = "versions";

pub fn version_dir(db_path: &Path, version: u64) -> PathBuf {
    db_path.join(VERSIONS_DIR).join(format!("{:010}", version))
}

pub fn read_current_version(db_path: &Path) -> Result<u64> {
    let current_path = db_path.join(CURRENT_FILE);
    if !current_path.exists() {
        return Ok(0);
    }
    let s = fs::read_to_string(&current_path)?;
    s.trim()
        .parse::<u64>()
        .map_err(|_| MrqError::IndexCorrupt("CURRENT contains non-numeric version".to_string()))
}

pub fn write_current_version(db_path: &Path, version: u64) -> Result<()> {
    let tmp = db_path.join("CURRENT.tmp");
    fs::write(&tmp, format!("{:010}\n", version))?;
    fs::rename(&tmp, db_path.join(CURRENT_FILE))?;
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[allow(clippy::too_many_arguments)]
pub fn write_version(
    db_path: &Path,
    version: u64,
    signatures: &[ImageSignature],
    metadata: &[DocumentMetadata],
    deleted_ids: &HashSet<ImageId>,
    applied_batches: &[String],
    image_size: u32,
    wavelet_top_k: usize,
) -> Result<VersionManifest> {
    let vdir = version_dir(db_path, version);
    fs::create_dir_all(&vdir)?;

    // Write metadata.jsonl
    let meta_path = vdir.join("metadata.jsonl");
    {
        use std::io::Write;
        let mut f = fs::File::create(&meta_path)?;
        for m in metadata {
            let line = serde_json::to_string(m).map_err(|e| MrqError::Serialize(e.to_string()))?;
            writeln!(f, "{}", line)?;
        }
    }

    // Write signatures.bin
    let sigs_path = vdir.join("signatures.bin");
    {
        let encoded =
            bincode::serialize(signatures).map_err(|e| MrqError::Serialize(e.to_string()))?;
        fs::write(&sigs_path, &encoded)?;
    }

    // Write deletes.bin
    let del_path = vdir.join("deletes.bin");
    {
        let ids: Vec<ImageId> = deleted_ids.iter().copied().collect();
        let encoded = bincode::serialize(&ids).map_err(|e| MrqError::Serialize(e.to_string()))?;
        fs::write(&del_path, &encoded)?;
    }

    // Build manifest
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut files = Vec::new();
    for (fname, path) in [
        ("metadata.jsonl", &meta_path),
        ("signatures.bin", &sigs_path),
        ("deletes.bin", &del_path),
    ] {
        let size_bytes = fs::metadata(path)?.len();
        let checksum = format!("sha256:{}", sha256_file(path)?);
        files.push(ManifestFile {
            path: fname.to_string(),
            size_bytes,
            checksum,
        });
    }

    let manifest = VersionManifest {
        format_version: 1,
        index_version: version,
        created_at_unix_ms: now_ms,
        image_size,
        wavelet_top_k,
        files,
        applied_batches: applied_batches.to_vec(),
    };

    let manifest_path = vdir.join("manifest.json");
    let manifest_json =
        serde_json::to_string_pretty(&manifest).map_err(|e| MrqError::Serialize(e.to_string()))?;
    fs::write(&manifest_path, manifest_json.as_bytes())?;

    Ok(manifest)
}

pub fn load_version(db_path: &Path, version: u64) -> Result<LoadedIndex> {
    let vdir = version_dir(db_path, version);
    let manifest_path = vdir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(MrqError::IndexCorrupt(format!(
            "manifest.json missing for version {}",
            version
        )));
    }

    let manifest_json = fs::read_to_string(&manifest_path)?;
    let manifest: VersionManifest =
        serde_json::from_str(&manifest_json).map_err(|e| MrqError::IndexCorrupt(e.to_string()))?;

    // Verify checksums
    for mf in &manifest.files {
        let fpath = vdir.join(&mf.path);
        let expected = &mf.checksum;
        let actual = format!("sha256:{}", sha256_file(&fpath)?);
        if &actual != expected {
            return Err(MrqError::ChecksumMismatch {
                path: mf.path.clone(),
            });
        }
    }

    // Load signatures
    let sigs_bytes = fs::read(vdir.join("signatures.bin"))?;
    let signatures: Vec<ImageSignature> =
        bincode::deserialize(&sigs_bytes).map_err(|e| MrqError::Serialize(e.to_string()))?;

    // Load metadata
    let meta_content = fs::read_to_string(vdir.join("metadata.jsonl"))?;
    let mut metadata = Vec::new();
    for line in meta_content.lines() {
        if line.is_empty() {
            continue;
        }
        let m: DocumentMetadata =
            serde_json::from_str(line).map_err(|e| MrqError::Serialize(e.to_string()))?;
        metadata.push(m);
    }

    // Load deletes
    let del_bytes = fs::read(vdir.join("deletes.bin"))?;
    let del_ids: Vec<ImageId> =
        bincode::deserialize(&del_bytes).map_err(|e| MrqError::Serialize(e.to_string()))?;
    let deleted_ids: HashSet<ImageId> = del_ids.into_iter().collect();

    Ok(LoadedIndex {
        version,
        signatures,
        metadata,
        deleted_ids,
        manifest,
    })
}
