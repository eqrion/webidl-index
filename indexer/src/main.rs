mod config;
mod fetch;
mod merge;
mod model {
    pub use common::model::*;
}
mod parse;
mod render;
mod search_index;
mod store;
mod versions;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
use sha2::Digest;

#[derive(Parser)]
#[command(name = "indexer", about = "Builds the browser WebIDL index")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Path to data/ (the checked-in JSON database).
    #[arg(long, global = true, default_value = "../data")]
    data_dir: PathBuf,

    /// Path to engines.toml.
    #[arg(long, global = true, default_value = "config/engines.toml")]
    config: PathBuf,

    /// Local git cache root (not checked in; reused across runs for speed).
    #[arg(long, global = true, default_value = "../.cache")]
    cache_dir: PathBuf,
}

#[derive(Subcommand)]
enum Command {
    /// Index every not-yet-indexed major version, for one engine or all.
    Index {
        #[arg(long)]
        engine: Option<String>,
        /// Stop after indexing this many new versions (per engine).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Index exactly one major version, regardless of whether it exists.
    IndexOne { engine: String, major: u32 },
    /// Print discovered majors and the source tag each maps to.
    ListVersions { engine: String },
    /// Re-hash every object and confirm every snapshot entry resolves.
    Verify,
    /// Regenerate every snapshot's `<version>.index.json` from objects
    /// already on disk (no network). Useful after changing what the search
    /// index contains, or for snapshots written before it existed.
    BackfillIndex,
    /// Emit a single fully-resolved JSON document (all definitions
    /// inlined, no hash refs) for ad-hoc analysis. With one snapshot,
    /// exports it as-is; with two or more, exports their merged common
    /// subset (see `merge.rs`).
    Export {
        /// Shorthand for a single snapshot: `export blink 145`.
        engine: Option<String>,
        version: Option<String>,
        /// Repeatable `engine:version` pair; pass 2+ for a merged export.
        #[arg(long = "input")]
        inputs: Vec<String>,
        /// Write here; if omitted, print to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::load(&cli.config)?;

    match cli.command {
        Command::Index { engine, limit } => {
            for (name, econf) in &cfg.engines {
                if let Some(want) = &engine
                    && want != name
                {
                    continue;
                }
                // One engine's failure (e.g. discovery hitting a rate limit)
                // shouldn't stop the others from being attempted.
                if let Err(e) = run_index(name, econf, &cli.data_dir, &cli.cache_dir, limit) {
                    eprintln!("{name}: aborted: {e:?}");
                }
            }
        }
        Command::IndexOne { engine, major } => {
            let econf = cfg
                .engines
                .get(&engine)
                .with_context(|| format!("unknown engine {engine}"))?;
            let tag = discover(econf)?
                .into_iter()
                .find(|t| t.major == major)
                .with_context(|| format!("no tag found for {engine} major {major}"))?;
            index_version(&engine, econf, &tag, &cli.data_dir, &cli.cache_dir)?;
        }
        Command::ListVersions { engine } => {
            let econf = cfg
                .engines
                .get(&engine)
                .with_context(|| format!("unknown engine {engine}"))?;
            for t in discover(econf)? {
                println!("{}\t{}", t.major, t.tag);
            }
        }
        Command::Verify => verify(&cli.data_dir)?,
        Command::BackfillIndex => backfill_index(&cli.data_dir)?,
        Command::Export { engine, version, inputs, out } => {
            run_export(&cli.data_dir, engine, version, inputs, out)?
        }
    }

    Ok(())
}

fn discover(econf: &config::EngineConfig) -> Result<Vec<versions::VersionTag>> {
    match &econf.version_discovery {
        config::VersionDiscovery::GitTags { tag_pattern } => {
            versions::git_tags_by_major(&econf.repo, tag_pattern)
        }
        config::VersionDiscovery::Chromiumdash => versions::chromium_stable_by_major(),
    }
}

fn run_index(
    engine: &str,
    econf: &config::EngineConfig,
    data_dir: &Path,
    cache_root: &Path,
    limit: Option<usize>,
) -> Result<()> {
    let mut tags = discover(econf)?;
    // Newest first: if a run is limited or interrupted, the most relevant
    // (most recent) versions land first rather than the oldest.
    tags.sort_by(|a, b| b.major.cmp(&a.major));
    println!("{engine}: {} majors discovered", tags.len());
    let mut indexed = 0;
    for tag in &tags {
        if store::snapshot_exists(data_dir, engine, &tag.major.to_string()) {
            continue;
        }
        if let Some(limit) = limit
            && indexed >= limit
        {
            println!("{engine}: hit limit of {limit}, stopping");
            break;
        }
        // A single version's fetch/parse failure (network hiccup, a git
        // quirk) shouldn't take down the rest of a long backfill run. It's
        // simply left un-indexed and picked up on the next run.
        if let Err(e) = index_version(engine, econf, tag, data_dir, cache_root) {
            eprintln!("{engine} {}: skipped, failed: {e:?}", tag.major);
            continue;
        }
        indexed += 1;
    }
    println!("{engine}: indexed {indexed} new version(s)");
    Ok(())
}

fn index_version(
    engine: &str,
    econf: &config::EngineConfig,
    tag: &versions::VersionTag,
    data_dir: &Path,
    cache_root: &Path,
) -> Result<()> {
    let version = tag.major.to_string();
    println!("{engine} {version}: fetching {}", tag.tag);

    let cache_dir = cache_root.join(engine);
    fetch::ensure_repo(&cache_dir, &econf.repo, &econf.idl_paths)?;
    let checkout = fetch::checkout_tag(&cache_dir, &tag.tag)?;
    let files = fetch::collect_files(&checkout.root, &econf.extensions, &econf.exclude_paths)?;
    println!("{engine} {version}: parsing {} files", files.len());

    let merged = parse::merge_files(&files);
    for err in &merged.errors {
        eprintln!(
            "{engine} {version}: parse error in {}: {}",
            err.file, err.message
        );
    }

    let mut entries = BTreeMap::new();
    for def in &merged.definitions {
        let (hash, bytes) = store::hash_definition(def)?;
        store::write_object(data_dir, &hash, &bytes)?;
        entries.insert(def.name().to_string(), hash);
    }
    let entry_count = entries.len();

    let snapshot = store::Snapshot {
        engine: engine.to_string(),
        version: version.clone(),
        source: store::SnapshotSource {
            repo: econf.repo.clone(),
            tag: tag.tag.clone(),
            commit: checkout.commit.clone(),
        },
        date: checkout.date.clone(),
        entries,
        parse_errors: merged
            .errors
            .iter()
            .map(|e| format!("{}: {}", e.file, e.message))
            .collect(),
    };
    store::write_snapshot(data_dir, &snapshot)?;
    store::write_search_index(data_dir, engine, &version, &merged.definitions)?;

    let mut manifest = store::load_manifest(data_dir)?;
    store::upsert_manifest_version(
        &mut manifest,
        engine,
        store::ManifestVersion {
            version: version.clone(),
            date: checkout.date.clone(),
            commit: checkout.commit.clone(),
        },
    );
    store::save_manifest(data_dir, &manifest)?;

    println!("{engine} {version}: {entry_count} definitions");
    Ok(())
}

fn verify(data_dir: &Path) -> Result<()> {
    let manifest = store::load_manifest(data_dir)?;
    let mut checked = 0;
    let mut problems = 0;
    for (engine, versions) in &manifest.engines {
        for v in versions {
            let snapshot = store::read_snapshot(data_dir, engine, &v.version)?;
            for (name, hash) in &snapshot.entries {
                checked += 1;
                let path = data_dir
                    .join("objects")
                    .join(&hash[0..2])
                    .join(format!("{hash}.json"));
                if !path.exists() {
                    eprintln!("missing object for {engine} {} {name}: {hash}", v.version);
                    problems += 1;
                    continue;
                }
                let bytes = std::fs::read(&path)?;
                let mut hasher = sha2::Sha256::new();
                hasher.update(&bytes);
                let actual = hex::encode(hasher.finalize());
                if &actual != hash {
                    eprintln!(
                        "hash mismatch for {}: expected {hash}, got {actual}",
                        path.display()
                    );
                    problems += 1;
                }
            }
        }
    }
    println!("verified {checked} entries, {problems} problem(s)");
    if problems > 0 {
        bail!("{problems} verification problem(s)");
    }
    Ok(())
}

fn backfill_index(data_dir: &Path) -> Result<()> {
    let manifest = store::load_manifest(data_dir)?;
    for (engine, versions) in &manifest.engines {
        for v in versions {
            let snapshot = store::read_snapshot(data_dir, engine, &v.version)?;
            let definitions = snapshot
                .entries
                .values()
                .map(|hash| store::read_object(data_dir, hash))
                .collect::<Result<Vec<_>>>()?;
            store::write_search_index(data_dir, engine, &v.version, &definitions)?;
            println!("{engine} {}: wrote index for {} definitions", v.version, definitions.len());
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct ExportInput {
    engine: String,
    version: String,
}

#[derive(Serialize)]
struct ExportedSnapshot {
    engine: String,
    version: String,
    date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<store::SnapshotSource>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    inputs: Vec<ExportInput>,
    definitions: Vec<model::Definition>,
}

fn resolve_snapshot(data_dir: &Path, engine: &str, version: &str) -> Result<BTreeMap<String, model::Definition>> {
    let snapshot = store::read_snapshot(data_dir, engine, version)?;
    snapshot
        .entries
        .into_iter()
        .map(|(name, hash)| {
            let def = store::read_object(data_dir, &hash)?;
            Ok((name, def))
        })
        .collect()
}

fn run_export(
    data_dir: &Path,
    engine: Option<String>,
    version: Option<String>,
    inputs: Vec<String>,
    out: Option<PathBuf>,
) -> Result<()> {
    let mut refs: Vec<(String, String)> = inputs
        .iter()
        .map(|s| {
            let (e, v) = s
                .split_once(':')
                .with_context(|| format!("--input expects engine:version, got {s:?}"))?;
            Ok((e.to_string(), v.to_string()))
        })
        .collect::<Result<_>>()?;
    if let (Some(e), Some(v)) = (engine, version) {
        refs.insert(0, (e, v));
    }
    if refs.is_empty() {
        bail!("export needs either `<engine> <version>` or one or more `--input engine:version`");
    }

    let resolved: Vec<BTreeMap<String, model::Definition>> = refs
        .iter()
        .map(|(e, v)| resolve_snapshot(data_dir, e, v))
        .collect::<Result<_>>()?;

    let definitions = if refs.len() == 1 {
        let mut defs: Vec<model::Definition> = resolved[0].values().cloned().collect();
        defs.sort_by(|a, b| a.name().cmp(b.name()));
        defs
    } else {
        merge::merge_snapshots(&resolved)
    };

    let export = if refs.len() == 1 {
        let (engine, version) = &refs[0];
        let snapshot = store::read_snapshot(data_dir, engine, version)?;
        ExportedSnapshot {
            engine: engine.clone(),
            version: version.clone(),
            date: snapshot.date,
            source: Some(snapshot.source),
            inputs: Vec::new(),
            definitions,
        }
    } else {
        ExportedSnapshot {
            engine: "merged".to_string(),
            version: String::new(),
            date: String::new(),
            source: None,
            inputs: refs
                .iter()
                .map(|(engine, version)| ExportInput { engine: engine.clone(), version: version.clone() })
                .collect(),
            definitions,
        }
    };

    let bytes = serde_json::to_vec_pretty(&export)?;
    match out {
        Some(path) => {
            std::fs::write(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;
            eprintln!("wrote {} definitions to {}", export.definitions.len(), path.display());
        }
        None => {
            use std::io::Write;
            std::io::stdout().write_all(&bytes)?;
            println!();
        }
    }
    Ok(())
}
