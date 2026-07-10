//! Loads `config/engines.toml`: per-engine repo location, IDL file
//! selection, and how to discover one tag per major version.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct EngineConfig {
    pub repo: String,
    pub idl_paths: Vec<String>,
    pub extensions: Vec<String>,
    /// Path prefixes (relative to the checkout root) to skip, e.g. a
    /// binding-generator's own synthetic test fixtures that aren't real
    /// Web Platform API surface.
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    pub version_discovery: VersionDiscovery,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "method", rename_all = "kebab-case")]
pub enum VersionDiscovery {
    GitTags { tag_pattern: String },
    Chromiumdash,
    /// Always re-index the tip of one branch as a single "current" snapshot,
    /// for evergreen sources with no browser-style major version.
    Branch { branch: String },
}

impl VersionDiscovery {
    pub fn is_evergreen(&self) -> bool {
        matches!(self, VersionDiscovery::Branch { .. })
    }
}

#[derive(Deserialize)]
pub struct Config {
    #[serde(flatten)]
    pub engines: BTreeMap<String, EngineConfig>,
}

pub fn load(path: &Path) -> Result<Config> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}
