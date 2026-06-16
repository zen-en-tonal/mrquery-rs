use std::{fs, path::Path};

use mrq_core::{MrqError, Result};

use crate::{
    store::{read_current_version, sha256_file, version_dir},
    types::{IndexVersion, VersionManifest},
};

pub struct SnapshotManifest {
    pub index_version: IndexVersion,
    pub format_version: u32,
    pub files: Vec<SnapshotFile>,
    pub checksum: String,
}

pub struct SnapshotFile {
    pub path: String,
    pub size_bytes: u64,
    pub checksum: String,
}

pub fn create_snapshot(index_path: &Path, output_path: &Path) -> Result<SnapshotManifest> {
    let version = read_current_version(index_path)?;
    if version == 0 {
        return Err(MrqError::NotFound(
            "No committed version to snapshot".to_string(),
        ));
    }

    let src_dir = version_dir(index_path, version);
    fs::create_dir_all(output_path)?;

    let mut files = Vec::new();
    for entry in fs::read_dir(&src_dir)? {
        let entry = entry?;
        let src = entry.path();
        if src.is_file() {
            let fname = src.file_name().unwrap().to_string_lossy().to_string();
            let dst = output_path.join(&fname);
            fs::copy(&src, &dst)?;
            let size_bytes = fs::metadata(&dst)?.len();
            let checksum = format!("sha256:{}", sha256_file(&dst)?);
            files.push(SnapshotFile {
                path: fname,
                size_bytes,
                checksum,
            });
        }
    }

    // Overall checksum = hash of all file checksums concatenated
    let combined: String = files
        .iter()
        .map(|f| f.checksum.as_str())
        .collect::<Vec<_>>()
        .join("|");
    let overall = format!("sha256:{}", {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(combined.as_bytes());
        hex::encode(h.finalize())
    });

    Ok(SnapshotManifest {
        index_version: version,
        format_version: 1,
        files,
        checksum: overall,
    })
}

pub fn load_snapshot(snapshot_path: &Path, index_path: &Path) -> Result<IndexVersion> {
    // Read manifest from snapshot
    let manifest_path = snapshot_path.join("manifest.json");
    let manifest_json = fs::read_to_string(&manifest_path)?;
    let manifest: VersionManifest =
        serde_json::from_str(&manifest_json).map_err(|e| MrqError::IndexCorrupt(e.to_string()))?;

    let version = manifest.index_version;
    let dst_dir = version_dir(index_path, version);
    fs::create_dir_all(&dst_dir)?;

    // Copy manifest.json itself first so load_version can open it
    fs::copy(&manifest_path, dst_dir.join("manifest.json"))?;

    for mf in &manifest.files {
        let src = snapshot_path.join(&mf.path);
        let dst = dst_dir.join(&mf.path);
        fs::copy(&src, &dst)?;
        // Verify checksum
        let actual = format!("sha256:{}", sha256_file(&dst)?);
        if actual != mf.checksum {
            return Err(MrqError::ChecksumMismatch {
                path: mf.path.clone(),
            });
        }
    }

    crate::store::write_current_version(index_path, version)?;
    Ok(version)
}
