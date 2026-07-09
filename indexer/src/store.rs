//! Content-addressed storage under `data/`: one JSON file per unique
//! definition (`objects/<shard>/<sha256>.json`), one small manifest per
//! (engine, version) pointing at those objects by name (`snapshots/`), and a
//! top-level `manifest.json` the frontend loads first.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::Definition;
use crate::search_index;

#[derive(Serialize, Deserialize, Clone)]
pub struct SnapshotSource {
    pub repo: String,
    pub tag: String,
    pub commit: String,
}

#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub engine: String,
    pub version: String,
    pub source: SnapshotSource,
    pub date: String,
    pub entries: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parse_errors: Vec<String>,
}

/// Hashes a definition's canonical JSON encoding. Returns the hex digest and
/// the bytes that were hashed, so callers can write the object without
/// re-serializing.
pub fn hash_definition(def: &Definition) -> Result<(String, Vec<u8>)> {
    let bytes = serde_json::to_vec(def).context("serializing definition")?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok((hex::encode(hasher.finalize()), bytes))
}

pub fn write_object(data_dir: &Path, hash: &str, bytes: &[u8]) -> Result<()> {
    let dir = data_dir.join("objects").join(&hash[0..2]);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{hash}.json"));
    if !path.exists() {
        std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}

pub fn read_object(data_dir: &Path, hash: &str) -> Result<Definition> {
    let path = data_dir.join("objects").join(&hash[0..2]).join(format!("{hash}.json"));
    let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
}

pub fn write_snapshot(data_dir: &Path, snapshot: &Snapshot) -> Result<()> {
    let dir = data_dir.join("snapshots").join(&snapshot.engine);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", snapshot.version));
    let bytes = serde_json::to_vec_pretty(snapshot)?;
    std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Writes the `<version>.index.json` search/exposure index alongside the
/// snapshot. Kept separate from the snapshot itself so loading the version
/// picker never has to pull this larger, only-sometimes-needed file.
pub fn write_search_index(
    data_dir: &Path,
    engine: &str,
    version: &str,
    definitions: &[Definition],
) -> Result<()> {
    let dir = data_dir.join("snapshots").join(engine);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{version}.index.json"));
    let entries = search_index::build(definitions);
    let bytes = serde_json::to_vec(&entries)?;
    std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

pub fn read_snapshot(data_dir: &Path, engine: &str, version: &str) -> Result<Snapshot> {
    let path = data_dir
        .join("snapshots")
        .join(engine)
        .join(format!("{version}.json"));
    let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn snapshot_exists(data_dir: &Path, engine: &str, version: &str) -> bool {
    data_dir
        .join("snapshots")
        .join(engine)
        .join(format!("{version}.json"))
        .exists()
}

#[derive(Serialize, Deserialize, Default)]
pub struct Manifest {
    #[serde(default)]
    pub engines: BTreeMap<String, Vec<ManifestVersion>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ManifestVersion {
    pub version: String,
    pub date: String,
    pub commit: String,
}

pub fn load_manifest(data_dir: &Path) -> Result<Manifest> {
    let path = data_dir.join("manifest.json");
    if !path.exists() {
        return Ok(Manifest::default());
    }
    let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn save_manifest(data_dir: &Path, manifest: &Manifest) -> Result<()> {
    let path = data_dir.join("manifest.json");
    let bytes = serde_json::to_vec_pretty(manifest)?;
    std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

pub fn upsert_manifest_version(manifest: &mut Manifest, engine: &str, entry: ManifestVersion) {
    let list = manifest.engines.entry(engine.to_string()).or_default();
    if let Some(existing) = list.iter_mut().find(|v| v.version == entry.version) {
        *existing = entry;
    } else {
        list.push(entry);
    }
    list.sort_by(|a, b| version_sort_key(&a.version).cmp(&version_sort_key(&b.version)));
}

/// Splits a version string into its numeric components so "9.0" sorts before
/// "10.0" (plain string sort would put "10.0" first).
fn version_sort_key(v: &str) -> Vec<u32> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}
