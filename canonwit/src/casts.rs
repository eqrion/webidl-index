//! WIT has no subtyping, so every WebIDL upcast/downcast becomes an
//! explicit generated function ("universal + direct downcasts", see the
//! plan): a per-interface upcast to `object` and downcast from `any`, the
//! two `object`/`any` bridges, and a direct one-hop downcast per
//! inheritance edge so the common in-hierarchy case doesn't have to
//! round-trip through `any`. O(interfaces + inheritance edges), not O(n^2).

use wit_encoder::{Interface, Params, StandaloneFunc, Type as WitType};

use crate::names::NameRegistry;
use crate::report::Report;
use crate::symbols::{SymbolTable, ANY, OBJECT};

fn infallible_cast(names: &mut NameRegistry, report: &mut Report, from: &str, to: &str) -> StandaloneFunc {
    let name = names.register(&format!("{from}-as-{to}"), report);
    let mut f = StandaloneFunc::new(name, false);
    let mut params = Params::empty();
    params.push("v", WitType::borrow(from.to_string()));
    f.set_params(params);
    f.set_result(Some(WitType::named(to.to_string())));
    f
}

fn fallible_cast(names: &mut NameRegistry, report: &mut Report, from: &str, to: &str) -> StandaloneFunc {
    let name = names.register(&format!("{from}-as-{to}"), report);
    let mut f = StandaloneFunc::new(name, false);
    let mut params = Params::empty();
    params.push("v", WitType::borrow(from.to_string()));
    f.set_params(params);
    f.set_result(Some(WitType::option(WitType::named(to.to_string()))));
    f
}

pub fn emit_casts(symbols: &SymbolTable, names: &mut NameRegistry, iface: &mut Interface, report: &mut Report) {
    iface.function(infallible_cast(names, report, OBJECT, ANY));
    iface.function(fallible_cast(names, report, ANY, OBJECT));

    let mut interfaces: Vec<&str> = symbols
        .interface_names()
        .filter_map(|name| symbols.wit_name(name))
        .collect();
    interfaces.sort_unstable();
    interfaces.dedup();
    for i in &interfaces {
        iface.function(infallible_cast(names, report, i, OBJECT));
        iface.function(fallible_cast(names, report, ANY, i));
    }

    let mut edges: Vec<(String, String)> = symbols
        .interface_edges()
        .into_iter()
        .filter_map(|(base, derived)| Some((symbols.wit_name(base)?.to_string(), symbols.wit_name(derived)?.to_string())))
        .collect();
    edges.sort_unstable();
    edges.dedup();
    for (base, derived) in &edges {
        iface.function(fallible_cast(names, report, base, derived));
    }
}
