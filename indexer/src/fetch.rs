//! Fetches the IDL subtree of a repo at one ref (tag or branch), without ever
//! cloning full history.
//!
//! A plain `--filter=blob:none --sparse` clone still walks the *entire*
//! commit history to build the clone (verified against
//! mozilla-firefox/firefox: this alone did not finish in two minutes). The
//! trick is to skip history entirely: `init` + `remote add` + sparse-checkout
//! the target paths + `fetch --depth 1 --filter=blob:none origin <ref>`.
//! That pulls exactly one commit and its trees, letting checkout lazily fetch
//! blobs only for the sparse paths. Verified end-to-end: ~2-7s per ref.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

pub struct Checkout {
    pub root: PathBuf,
    pub commit: String,
    pub date: String,
}

/// Sets up `cache_dir` as a fresh git repo with `origin` pointing at
/// `repo_url` and `sparse_paths` configured. Always reinitializes: reusing
/// one shallow repo across many sequential `--depth 1` fetches of unrelated
/// commits eventually corrupts git's shallow-boundary bookkeeping ("fatal:
/// shallow file has changed since we read it"), confirmed after ~30
/// sequential fetches into the same repo during a real backfill run. A
/// fresh init per version costs a few redundant small object downloads;
/// that's cheaper than a run dying partway through.
pub fn ensure_repo(cache_dir: &Path, repo_url: &str, sparse_paths: &[String]) -> Result<()> {
    if cache_dir.exists() {
        std::fs::remove_dir_all(cache_dir)
            .with_context(|| format!("removing {}", cache_dir.display()))?;
    }
    std::fs::create_dir_all(cache_dir)?;
    run_git(cache_dir, &["init", "-q"])?;
    run_git(cache_dir, &["remote", "add", "origin", repo_url])?;

    let mut args: Vec<&str> = vec!["sparse-checkout", "set", "--no-cone"];
    for p in sparse_paths {
        args.push(p.as_str());
    }
    run_git(cache_dir, &args)?;
    Ok(())
}

/// Fetches and checks out `ref_`, which may be a tag or a branch name.
/// `cache_dir` must already be set up via `ensure_repo`.
pub fn checkout_ref(cache_dir: &Path, ref_: &str) -> Result<Checkout> {
    run_git(
        cache_dir,
        &["fetch", "--depth", "1", "--filter=blob:none", "origin", ref_],
    )?;
    run_git(cache_dir, &["checkout", "--detach", "--force", "FETCH_HEAD"])?;
    let commit = run_git_capture(cache_dir, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    let date = run_git_capture(cache_dir, &["log", "-1", "--format=%aI", "HEAD"])?
        .trim()
        .to_string();
    Ok(Checkout {
        root: cache_dir.to_path_buf(),
        commit,
        date,
    })
}

/// Walks the checkout for files with one of `extensions`, returning
/// `(path relative to root, contents)` sorted by path for determinism.
/// Skips any path starting with one of `exclude_paths`.
pub fn collect_files(
    root: &Path,
    extensions: &[String],
    exclude_paths: &[String],
) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !extensions.iter().any(|e| e == ext) {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .into_owned();
        if exclude_paths.iter().any(|p| rel.starts_with(p)) {
            continue;
        }
        let content = std::fs::read_to_string(entry.path())
            .with_context(|| format!("reading {}", entry.path().display()))?;
        out.push((rel, content));
    }
    out.sort();
    Ok(out)
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .with_context(|| format!("running git {args:?} in {}", cwd.display()))?;
    if !status.success() {
        bail!("git {args:?} failed in {}", cwd.display());
    }
    Ok(())
}

fn run_git_capture(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("running git {args:?} in {}", cwd.display()))?;
    if !output.status.success() {
        bail!(
            "git {args:?} failed in {}: {}",
            cwd.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
