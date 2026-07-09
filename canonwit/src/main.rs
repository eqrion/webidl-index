use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use canonwit::input::ExportedSnapshot;
use canonwit::{convert, ConvertOptions};

/// Converts a WebIDL snapshot (as exported by `indexer export`) into a
/// single canonical WIT file, using heuristics to invert the
/// component-to-WebIDL `CanonicalWebIDLType` mapping.
#[derive(Parser)]
#[command(name = "canonwit")]
struct Cli {
    /// Path to an exported snapshot JSON file, or `-` for stdin.
    input: String,

    /// Write the WIT here; if omitted, print to stdout.
    #[arg(short, long)]
    out: Option<PathBuf>,

    /// Package namespace, e.g. `web` in `web:web`.
    #[arg(long, default_value = "web")]
    package_namespace: String,

    /// Package name, e.g. `web` in `web:web`.
    #[arg(long, default_value = "web")]
    package_name: String,

    /// Overrides the snapshot's own version for the package version.
    #[arg(long)]
    package_version: Option<String>,

    /// Name of the single emitted interface.
    #[arg(long, default_value = "web")]
    interface_name: String,

    /// Also emit a `world` importing the interface.
    #[arg(long)]
    world: bool,

    /// Write a machine-readable JSON report of every heuristic decision.
    #[arg(long)]
    report: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let raw = if cli.input == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).context("reading snapshot from stdin")?;
        buf
    } else {
        std::fs::read_to_string(&cli.input).with_context(|| format!("reading {}", cli.input))?
    };
    let snapshot: ExportedSnapshot = serde_json::from_str(&raw).context("parsing snapshot JSON")?;

    let opts = ConvertOptions {
        package_namespace: cli.package_namespace,
        package_name: cli.package_name,
        package_version: cli.package_version,
        interface_name: cli.interface_name,
        emit_world: cli.world,
    };

    let (wit, report) = convert(snapshot, &opts)?;

    match cli.out {
        Some(path) => {
            std::fs::write(&path, &wit).with_context(|| format!("writing {}", path.display()))?;
            eprintln!("wrote {}", path.display());
        }
        None => println!("{wit}"),
    }

    report.print_summary();
    if let Some(path) = cli.report {
        let json = serde_json::to_vec_pretty(&report).context("serializing report")?;
        std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
    }

    Ok(())
}
