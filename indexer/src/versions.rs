//! Discovers one representative tag per major version for each engine.
//!
//! Gecko and WebKit both tag every release in git, so we list remote tags and
//! bucket them by the major version captured out of the tag name. Chromium
//! also tags every release, but the tag *names* are opaque version strings
//! with no separate "major" marker -- ChromiumDash already tracks the
//! milestone -> version mapping we'd otherwise have to reconstruct, so we use
//! its API instead of guessing from ~40k tags.

use std::collections::BTreeMap;
use std::process::Command;

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::Deserialize;

pub struct VersionTag {
    pub major: u32,
    pub tag: String,
}

/// Lists remote tags matching `pattern` (which must have exactly one capture
/// group: the major version number) and keeps the first tag seen per major.
/// "First seen" is fine here because release tags are created in order and
/// we only need *a* representative snapshot per major, not the exact GA tag.
pub fn git_tags_by_major(repo_url: &str, pattern: &str) -> Result<Vec<VersionTag>> {
    let re = Regex::new(pattern).context("compiling tag pattern")?;
    let output = Command::new("git")
        .args(["ls-remote", "--tags", repo_url])
        .output()
        .context("running git ls-remote")?;
    if !output.status.success() {
        bail!(
            "git ls-remote failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut by_major: BTreeMap<u32, String> = BTreeMap::new();
    for line in text.lines() {
        let Some(tag) = line
            .split('\t')
            .nth(1)
            .and_then(|r| r.strip_prefix("refs/tags/"))
        else {
            continue;
        };
        if tag.ends_with("^{}") {
            continue;
        }
        let Some(caps) = re.captures(tag) else {
            continue;
        };
        let Some(major) = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) else {
            continue;
        };
        by_major.entry(major).or_insert_with(|| tag.to_string());
    }
    Ok(by_major
        .into_iter()
        .map(|(major, tag)| VersionTag { major, tag })
        .collect())
}

#[derive(Deserialize)]
struct ChromiumRelease {
    milestone: u32,
    version: String,
    time: f64,
}

/// Queries ChromiumDash for every Stable release and keeps the earliest
/// (lowest `time`) version string per milestone -- that's the GA release for
/// that major, and its version string doubles as the git tag on
/// github.com/chromium/chromium (verified: tag `150.0.7871.114` matches the
/// commit ChromiumDash reports for milestone 150).
pub fn chromium_stable_by_major() -> Result<Vec<VersionTag>> {
    // 1000 is the API's own maximum (larger values 400). Chromium's entire
    // Stable/Linux release history is under 600 entries, so this covers it.
    let url = "https://chromiumdash.appspot.com/fetch_releases?channel=Stable&platform=Linux&num=1000";
    let body = ureq::get(url)
        .call()
        .context("requesting chromiumdash releases")?
        .body_mut()
        .read_to_string()
        .context("reading chromiumdash response")?;
    let releases: Vec<ChromiumRelease> =
        serde_json::from_str(&body).context("parsing chromiumdash response")?;

    let mut earliest: BTreeMap<u32, (String, f64)> = BTreeMap::new();
    for r in releases {
        earliest
            .entry(r.milestone)
            .and_modify(|(v, t)| {
                if r.time < *t {
                    *v = r.version.clone();
                    *t = r.time;
                }
            })
            .or_insert((r.version.clone(), r.time));
    }
    Ok(earliest
        .into_iter()
        .map(|(major, (version, _))| VersionTag { major, tag: version })
        .collect())
}
