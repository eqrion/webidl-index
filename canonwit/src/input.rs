//! Mirrors the JSON shape `indexer export` writes (`ExportedSnapshot` in
//! `indexer/src/main.rs`). We only need `engine`/`version` (for a default
//! package name/version) and the inlined `definitions`; unused fields
//! (`date`, `source`, `inputs`) are ignored by serde automatically.

use common::model::Definition;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ExportedSnapshot {
    pub engine: String,
    #[serde(default)]
    pub version: String,
    pub definitions: Vec<Definition>,
}
