//! Pass 1: classify every top-level WebIDL name and assign it its final
//! kebab-case WIT name, so the type mapper (identifier references), the
//! namespace/cast function emitters, and the definition emitter all agree
//! on one name per WebIDL symbol.

use std::collections::{HashMap, HashSet};

use common::model::Definition;

use crate::names::NameRegistry;
use crate::report::Report;

/// Every maximal run of identifier characters in a type string, e.g.
/// `"sequence<AuctionAdConfig>"` -> `["sequence", "AuctionAdConfig"]`. Cheap
/// over-approximation (keywords are picked up too) used only to build the
/// dictionary/typedef reference graph below, where we immediately filter
/// against a known name set.
fn extract_identifiers(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            out.push(std::str::from_utf8(&bytes[start..i]).unwrap());
        } else {
            i += 1;
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Interface,
    CallbackInterface,
    Namespace,
    Dictionary,
    Enum,
    Typedef,
    Callback,
}

/// The kebab names of the three shared opaque builtin resources. Pre-claimed
/// in the shared registry so a same-named WebIDL definition (unlikely, but
/// possible) gets renamed instead of them.
pub const ANY: &str = "any";
pub const OBJECT: &str = "object";
pub const SYMBOL: &str = "symbol";

pub struct SymbolTable {
    kinds: HashMap<String, SymbolKind>,
    /// interface/dictionary name -> its `inherits`/base name.
    bases: HashMap<String, String>,
    /// WebIDL name -> assigned WIT kebab name, for symbols that become a
    /// named WIT type (Interface/Dictionary/Enum/Typedef).
    wit_names: HashMap<String, String>,
    /// WIT kebab names that denote a `resource` (real interfaces plus the
    /// three opaque builtins) -- used to decide `borrow<r>` vs owned `r` at
    /// parameter positions.
    resource_names: HashSet<String>,
    /// `(dictionary/typedef, referenced dictionary/typedef)` edges that
    /// close a cycle in the "stored by value" type graph -- WIT records
    /// can't be recursive (even through `list<T>`), unlike WebIDL
    /// dictionaries. `emit.rs` substitutes `object` at exactly these edges.
    cyclic_edges: HashSet<(String, String)>,
}

impl SymbolTable {
    /// Builds the symbol table and the shared top-level `NameRegistry`
    /// (pre-loaded with every name assigned here). Returned separately so
    /// later passes can hold an immutable `&SymbolTable` for lookups
    /// alongside a mutable `&mut NameRegistry` for new registrations
    /// (union names, namespace functions, generated casts) without a borrow
    /// conflict.
    pub fn build(defs: &[Definition], report: &mut Report) -> (Self, NameRegistry) {
        let mut kinds = HashMap::new();
        let mut bases = HashMap::new();
        let mut wit_names = HashMap::new();
        let mut resource_names = HashSet::new();
        let mut names = NameRegistry::new();

        for builtin in [ANY, OBJECT, SYMBOL] {
            resource_names.insert(names.register(builtin, report));
        }

        for def in defs {
            let (kind, inherits) = match def {
                Definition::Interface { inherits, .. } => (SymbolKind::Interface, inherits.clone()),
                Definition::CallbackInterface { .. } => (SymbolKind::CallbackInterface, None),
                Definition::Namespace { .. } => (SymbolKind::Namespace, None),
                Definition::Dictionary { inherits, .. } => (SymbolKind::Dictionary, inherits.clone()),
                Definition::Enum { .. } => (SymbolKind::Enum, None),
                Definition::Typedef { .. } => (SymbolKind::Typedef, None),
                Definition::Callback { .. } => (SymbolKind::Callback, None),
            };
            let name = def.name().to_string();
            if matches!(
                kind,
                SymbolKind::Interface | SymbolKind::Dictionary | SymbolKind::Enum | SymbolKind::Typedef
            ) {
                let wit_name = names.register(&name, report);
                if matches!(kind, SymbolKind::Interface) {
                    resource_names.insert(wit_name.clone());
                }
                wit_names.insert(name.clone(), wit_name);
            }
            kinds.insert(name.clone(), kind);
            if let Some(base) = inherits {
                bases.insert(name, base);
            }
        }

        let cyclic_edges = find_cyclic_edges(defs, &kinds);

        (Self { kinds, bases, wit_names, resource_names, cyclic_edges }, names)
    }

    pub fn kind_of(&self, name: &str) -> Option<SymbolKind> {
        self.kinds.get(name).copied()
    }

    pub fn base_of(&self, name: &str) -> Option<&str> {
        self.bases.get(name).map(String::as_str)
    }

    /// The assigned WIT kebab name for a symbol that becomes a named WIT
    /// type. `None` for namespaces/callbacks/unknown identifiers.
    pub fn wit_name(&self, name: &str) -> Option<&str> {
        self.wit_names.get(name).map(String::as_str)
    }

    /// Whether a WIT kebab name (as produced by `wit_name`, or one of the
    /// builtin opaque names) denotes a `resource`.
    pub fn is_resource_wit_name(&self, wit_name: &str) -> bool {
        self.resource_names.contains(wit_name)
    }

    /// Whether resolving `to` while lowering `from`'s fields/alias would
    /// close a cycle in the stored-by-value type graph.
    pub fn is_cyclic_edge(&self, from: &str, to: &str) -> bool {
        self.cyclic_edges.contains(&(from.to_string(), to.to_string()))
    }

    /// The first identifier in `type_str` that would close a cycle if
    /// resolved while lowering `from` (a dictionary field or typedef
    /// target). `emit.rs` substitutes `object` for the whole type when this
    /// returns `Some`.
    pub fn cyclic_ref_in(&self, from: &str, type_str: &str) -> Option<String> {
        extract_identifiers(type_str)
            .into_iter()
            .find(|id| self.is_cyclic_edge(from, id))
            .map(str::to_string)
    }

    /// Every WebIDL name classified as an interface.
    pub fn interface_names(&self) -> impl Iterator<Item = &str> {
        self.kinds
            .iter()
            .filter(|(_, kind)| **kind == SymbolKind::Interface)
            .map(|(name, _)| name.as_str())
    }

    /// Every `(base, derived)` inheritance edge among interfaces only
    /// (dictionary bases are flattened directly in `emit.rs` and don't need
    /// a cast function).
    pub fn interface_edges(&self) -> Vec<(&str, &str)> {
        self.bases
            .iter()
            .filter(|(derived, _)| self.kinds.get(*derived) == Some(&SymbolKind::Interface))
            .map(|(derived, base)| (base.as_str(), derived.as_str()))
            .collect()
    }

    /// The full ancestor chain of `name`, nearest base first.
    pub fn ancestors(&self, name: &str) -> Vec<&str> {
        let mut chain = Vec::new();
        let mut cur = name;
        while let Some(base) = self.bases.get(cur) {
            chain.push(base.as_str());
            cur = base;
        }
        chain
    }
}

/// Builds the dictionary/typedef reference graph (edges are direct
/// field/alias type references, regardless of `list`/`option` wrapping --
/// WIT can't store a record inside itself even indirectly through a list)
/// and returns every edge that closes a cycle: a DFS with an explicit
/// recursion stack, marking each edge into a node still on the stack as
/// cyclic. Removing exactly these edges leaves a DAG.
fn find_cyclic_edges(defs: &[Definition], kinds: &HashMap<String, SymbolKind>) -> HashSet<(String, String)> {
    let is_stored_by_value =
        |name: &str| matches!(kinds.get(name), Some(SymbolKind::Dictionary | SymbolKind::Typedef));

    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for def in defs {
        let (name, type_strs): (&str, Vec<&str>) = match def {
            Definition::Dictionary { name, fields, .. } => {
                (name.as_str(), fields.iter().map(|f| f.type_.as_str()).collect())
            }
            Definition::Typedef { name, aliased_type, .. } => (name.as_str(), vec![aliased_type.as_str()]),
            _ => continue,
        };
        let refs: Vec<&str> = type_strs
            .iter()
            .flat_map(|s| extract_identifiers(s))
            .filter(|id| is_stored_by_value(id))
            .collect();
        adjacency.entry(name).or_default().extend(refs);
    }

    let mut cyclic = HashSet::new();
    let mut state: HashMap<&str, u8> = HashMap::new(); // 0=unvisited, 1=on stack, 2=done

    fn visit<'a>(
        node: &'a str,
        adjacency: &HashMap<&'a str, Vec<&'a str>>,
        state: &mut HashMap<&'a str, u8>,
        cyclic: &mut HashSet<(String, String)>,
    ) {
        state.insert(node, 1);
        if let Some(refs) = adjacency.get(node) {
            for &next in refs {
                match state.get(next).copied().unwrap_or(0) {
                    1 => {
                        cyclic.insert((node.to_string(), next.to_string()));
                    }
                    0 => visit(next, adjacency, state, cyclic),
                    _ => {}
                }
            }
        }
        state.insert(node, 2);
    }

    for &node in adjacency.keys() {
        if state.get(node).copied().unwrap_or(0) == 0 {
            visit(node, &adjacency, &mut state, &mut cyclic);
        }
    }
    cyclic
}
