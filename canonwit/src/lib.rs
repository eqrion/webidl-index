pub mod casts;
pub mod emit;
pub mod input;
pub mod names;
pub mod report;
pub mod symbols;
pub mod typing;
pub mod unions;

use anyhow::Result;
use semver::Version;
use wit_encoder::{Interface, Package, PackageName};

use crate::input::ExportedSnapshot;
use crate::report::Report;
use crate::symbols::SymbolTable;

pub struct ConvertOptions {
    pub package_namespace: String,
    pub package_name: String,
    /// Overrides the snapshot's own version, if given.
    pub package_version: Option<String>,
    pub interface_name: String,
    pub emit_world: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            package_namespace: "web".to_string(),
            package_name: "web".to_string(),
            package_version: None,
            interface_name: "web".to_string(),
            emit_world: false,
        }
    }
}

fn parse_package_version(raw: &str) -> Option<Version> {
    let major: u64 = raw.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()?;
    Some(Version::new(major, 0, 0))
}

pub fn convert(snapshot: ExportedSnapshot, opts: &ConvertOptions) -> Result<(String, Report)> {
    let mut report = Report::default();
    let (symbols, mut names) = SymbolTable::build(&snapshot.definitions, &mut report);

    let mut iface = Interface::new(opts.interface_name.clone());
    emit::lower_all(&snapshot.definitions, &symbols, &mut names, &mut iface, &mut report);
    casts::emit_casts(&symbols, &mut names, &mut iface, &mut report);

    let version_str = opts.package_version.as_deref().unwrap_or(&snapshot.version);
    let version = parse_package_version(version_str);
    let pkg_name = PackageName::new(opts.package_namespace.clone(), opts.package_name.clone(), version);
    let mut pkg = Package::new(pkg_name);
    pkg.interface(iface);

    if opts.emit_world {
        let mut world = wit_encoder::World::new(format!("{}-imports", opts.interface_name));
        world.named_interface_import(opts.interface_name.clone());
        pkg.world(world);
    }

    Ok((pkg.to_string(), report))
}
