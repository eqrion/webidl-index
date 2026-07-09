//! Computes the common subset ("merged") `Definition`s shared by several
//! resolved snapshots. Pure logic, no I/O -- callers in `main.rs` resolve
//! `Snapshot.entries` into `BTreeMap<String, Definition>` and pass those in.
//!
//! This is the Rust half of the merge feature; `web/src/merge.ts` implements
//! the same semantics for the client-side live view. Keep the two in lockstep
//! -- the unit tests below are the shared reference cases.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{Argument, Definition, Field, Member};

/// Intersects the same-named `Definition` drawn from every input snapshot.
/// A name absent from any input, or present with a different `kind`, is
/// dropped -- it isn't common surface.
pub fn merge_snapshots(inputs: &[BTreeMap<String, Definition>]) -> Vec<Definition> {
    if inputs.is_empty() {
        return Vec::new();
    }

    let mut common_names: BTreeSet<String> = inputs[0].keys().cloned().collect();
    for map in &inputs[1..] {
        let keys: BTreeSet<String> = map.keys().cloned().collect();
        common_names = common_names.intersection(&keys).cloned().collect();
    }

    let mut out = Vec::new();
    for name in common_names {
        let defs: Vec<&Definition> = inputs.iter().map(|m| &m[&name]).collect();
        if let Some(mut merged) = merge_group(&defs) {
            merged.canonicalize();
            out.push(merged);
        }
    }
    out.sort_by(|a, b| a.name().cmp(b.name()));
    out
}

fn merge_group(defs: &[&Definition]) -> Option<Definition> {
    let name = defs[0].name().to_string();
    match defs[0] {
        Definition::Interface { .. } => {
            let mut inherits_per_input = Vec::new();
            let mut members_per_input: Vec<&[Member]> = Vec::new();
            let mut attrs_per_input: Vec<Vec<String>> = Vec::new();
            for d in defs.iter().copied() {
                match d {
                    Definition::Interface {
                        inherits,
                        extended_attributes,
                        members,
                        ..
                    } => {
                        inherits_per_input.push(inherits.clone());
                        members_per_input.push(members.as_slice());
                        attrs_per_input.push(extended_attributes.clone());
                    }
                    _ => return None,
                }
            }
            Some(Definition::Interface {
                name,
                inherits: agree_or_drop(&inherits_per_input),
                extended_attributes: intersect_extended_attributes(&attrs_per_input),
                members: merge_members(&members_per_input),
            })
        }
        Definition::CallbackInterface { .. } => {
            let mut members_per_input: Vec<&[Member]> = Vec::new();
            let mut attrs_per_input: Vec<Vec<String>> = Vec::new();
            for d in defs.iter().copied() {
                match d {
                    Definition::CallbackInterface {
                        extended_attributes,
                        members,
                        ..
                    } => {
                        members_per_input.push(members.as_slice());
                        attrs_per_input.push(extended_attributes.clone());
                    }
                    _ => return None,
                }
            }
            Some(Definition::CallbackInterface {
                name,
                extended_attributes: intersect_extended_attributes(&attrs_per_input),
                members: merge_members(&members_per_input),
            })
        }
        Definition::Namespace { .. } => {
            let mut members_per_input: Vec<&[Member]> = Vec::new();
            let mut attrs_per_input: Vec<Vec<String>> = Vec::new();
            for d in defs.iter().copied() {
                match d {
                    Definition::Namespace {
                        extended_attributes,
                        members,
                        ..
                    } => {
                        members_per_input.push(members.as_slice());
                        attrs_per_input.push(extended_attributes.clone());
                    }
                    _ => return None,
                }
            }
            Some(Definition::Namespace {
                name,
                extended_attributes: intersect_extended_attributes(&attrs_per_input),
                members: merge_members(&members_per_input),
            })
        }
        Definition::Dictionary { .. } => {
            let mut inherits_per_input = Vec::new();
            let mut fields_per_input: Vec<&[Field]> = Vec::new();
            let mut attrs_per_input: Vec<Vec<String>> = Vec::new();
            for d in defs.iter().copied() {
                match d {
                    Definition::Dictionary {
                        inherits,
                        extended_attributes,
                        fields,
                        ..
                    } => {
                        inherits_per_input.push(inherits.clone());
                        fields_per_input.push(fields.as_slice());
                        attrs_per_input.push(extended_attributes.clone());
                    }
                    _ => return None,
                }
            }
            Some(Definition::Dictionary {
                name,
                inherits: agree_or_drop(&inherits_per_input),
                extended_attributes: intersect_extended_attributes(&attrs_per_input),
                fields: merge_fields(&fields_per_input),
            })
        }
        Definition::Enum { .. } => {
            let mut values_per_input: Vec<&[String]> = Vec::new();
            let mut attrs_per_input: Vec<Vec<String>> = Vec::new();
            for d in defs.iter().copied() {
                match d {
                    Definition::Enum {
                        extended_attributes,
                        values,
                        ..
                    } => {
                        values_per_input.push(values.as_slice());
                        attrs_per_input.push(extended_attributes.clone());
                    }
                    _ => return None,
                }
            }
            let mut common: BTreeSet<String> = values_per_input[0].iter().cloned().collect();
            for values in &values_per_input[1..] {
                let set: BTreeSet<String> = values.iter().cloned().collect();
                common = common.intersection(&set).cloned().collect();
            }
            Some(Definition::Enum {
                name,
                extended_attributes: intersect_extended_attributes(&attrs_per_input),
                values: common.into_iter().collect(),
            })
        }
        Definition::Typedef { .. } => {
            let mut aliased_per_input = Vec::new();
            let mut attrs_per_input: Vec<Vec<String>> = Vec::new();
            for d in defs.iter().copied() {
                match d {
                    Definition::Typedef {
                        extended_attributes,
                        aliased_type,
                        ..
                    } => {
                        aliased_per_input.push(aliased_type.clone());
                        attrs_per_input.push(extended_attributes.clone());
                    }
                    _ => return None,
                }
            }
            if aliased_per_input.iter().any(|a| *a != aliased_per_input[0]) {
                return None;
            }
            Some(Definition::Typedef {
                name,
                extended_attributes: intersect_extended_attributes(&attrs_per_input),
                aliased_type: aliased_per_input[0].clone(),
            })
        }
        Definition::Callback { .. } => {
            let mut return_type_per_input = Vec::new();
            let mut args_per_input: Vec<&[Argument]> = Vec::new();
            let mut attrs_per_input: Vec<Vec<String>> = Vec::new();
            for d in defs.iter().copied() {
                match d {
                    Definition::Callback {
                        extended_attributes,
                        return_type,
                        arguments,
                        ..
                    } => {
                        return_type_per_input.push(return_type.clone());
                        args_per_input.push(arguments.as_slice());
                        attrs_per_input.push(extended_attributes.clone());
                    }
                    _ => return None,
                }
            }
            if return_type_per_input
                .iter()
                .any(|r| *r != return_type_per_input[0])
            {
                return None;
            }
            let first_sig = argument_signature(args_per_input[0]);
            if args_per_input
                .iter()
                .any(|args| argument_signature(args) != first_sig)
            {
                return None;
            }
            Some(Definition::Callback {
                name,
                extended_attributes: intersect_extended_attributes(&attrs_per_input),
                return_type: return_type_per_input[0].clone(),
                arguments: args_per_input[0].to_vec(),
            })
        }
    }
}

/// `Some(x)` iff every input agrees on `x`; disagreement (or any input
/// having `None`) drops the value rather than picking a side.
fn agree_or_drop(values: &[Option<String>]) -> Option<String> {
    let first = values[0].as_ref()?;
    if values.iter().all(|v| v.as_deref() == Some(first.as_str())) {
        Some(first.clone())
    } else {
        None
    }
}

fn argument_signature(args: &[Argument]) -> Vec<String> {
    args.iter()
        .map(|a| format!("{}{}{}", a.type_, if a.optional { "?" } else { "" }, if a.variadic { "..." } else { "" }))
        .collect()
}

const SPECIAL_MODIFIERS: &[&str] = &["getter", "setter", "deleter", "legacycaller", "stringifier", "static"];

/// Identity used to decide "the same member" across snapshots. Argument
/// *names*, defaults, and per-argument extended attributes are excluded --
/// they legitimately drift between engines. Overloads stay distinct because
/// the argument type signature is part of the key; unnamed members
/// (constructors, anonymous getters/setters, iterable/maplike/setlike/
/// stringifier) are distinguished by their special modifiers and, where the
/// name is empty, by the rendered `type_` (e.g. `iterable<V>` vs
/// `iterable<K, V>`).
fn member_identity(m: &Member) -> String {
    let arg_sig = argument_signature(&m.arguments).join(",");
    let mut specials: Vec<&str> = m
        .modifiers
        .iter()
        .map(String::as_str)
        .filter(|m| SPECIAL_MODIFIERS.contains(m))
        .collect();
    specials.sort_unstable();
    let disambiguator = if m.name.is_empty() { m.type_.clone().unwrap_or_default() } else { String::new() };
    format!("{:?}\u{0}{}\u{0}{}\u{0}{}\u{0}{}", m.kind, m.name, arg_sig, specials.join(" "), disambiguator)
}

fn merge_members(per_input: &[&[Member]]) -> Vec<Member> {
    if per_input.is_empty() {
        return Vec::new();
    }

    let maps: Vec<BTreeMap<String, &Member>> = per_input
        .iter()
        .map(|members| members.iter().map(|m| (member_identity(m), m)).collect())
        .collect();

    let mut common_keys: BTreeSet<String> = maps[0].keys().cloned().collect();
    for map in &maps[1..] {
        let keys: BTreeSet<String> = map.keys().cloned().collect();
        common_keys = common_keys.intersection(&keys).cloned().collect();
    }

    let mut out = Vec::new();
    for key in common_keys {
        let matched: Vec<&Member> = maps.iter().map(|m| m[&key]).collect();
        out.push(merge_matched_members(&matched));
    }
    out.sort();
    out
}

/// Picks a deterministic representative (smallest canonical JSON encoding,
/// so the result doesn't depend on input ordering) and overrides only its
/// `extended_attributes` with the intersection across the matched members.
fn merge_matched_members(matched: &[&Member]) -> Member {
    let representative = matched
        .iter()
        .min_by_key(|m| serde_json::to_vec(m).unwrap_or_default())
        .expect("matched is non-empty");
    let mut merged = (*representative).clone();
    let attrs_per_input: Vec<Vec<String>> = matched.iter().map(|m| m.extended_attributes.clone()).collect();
    merged.extended_attributes = intersect_extended_attributes(&attrs_per_input);
    merged
}

fn merge_fields(per_input: &[&[Field]]) -> Vec<Field> {
    if per_input.is_empty() {
        return Vec::new();
    }

    let maps: Vec<BTreeMap<String, &Field>> = per_input
        .iter()
        .map(|fields| fields.iter().map(|f| (f.name.clone(), f)).collect())
        .collect();

    let mut common_names: BTreeSet<String> = maps[0].keys().cloned().collect();
    for map in &maps[1..] {
        let keys: BTreeSet<String> = map.keys().cloned().collect();
        common_names = common_names.intersection(&keys).cloned().collect();
    }

    let mut out = Vec::new();
    for name in common_names {
        let matched: Vec<&Field> = maps.iter().map(|m| m[&name]).collect();
        // A field whose type disagrees across inputs isn't the same common
        // field, even though the name matches.
        if matched.iter().any(|f| f.type_ != matched[0].type_) {
            continue;
        }
        let required = matched.iter().all(|f| f.required);
        let default = if matched.iter().all(|f| f.default == matched[0].default) {
            matched[0].default.clone()
        } else {
            None
        };
        let attrs_per_input: Vec<Vec<String>> = matched.iter().map(|f| f.extended_attributes.clone()).collect();
        out.push(Field {
            name,
            type_: matched[0].type_.clone(),
            required,
            default,
            extended_attributes: intersect_extended_attributes(&attrs_per_input),
        });
    }
    out.sort();
    out
}

/// Extended attributes are opaque strings (see `render.rs`), e.g.
/// `SecureContext`, `Exposed=Window`, `Exposed=(Window, Worker)`,
/// `Exposed=*`. Plain set intersection is wrong for the keyed, list-valued
/// forms: `Exposed=(Window, Worker)` and `Exposed=Window` would never match
/// and the attribute would silently disappear from the merged definition --
/// which breaks the frontend's "explore from a global" feature, since it
/// keys off `Exposed`/`Global`. So keyed attributes (`Key=Value`,
/// `Key=(a, b)`, `Key=*`) are parsed and their value sets are intersected
/// per key instead; everything else (plain flags, and the rarer
/// `Key=Ident(args)` form) is treated as an opaque string and intersected
/// by exact match.
pub fn intersect_extended_attributes(lists: &[Vec<String>]) -> Vec<String> {
    let mut plain: Option<BTreeSet<String>> = None;
    let mut keyed: Option<BTreeMap<String, ValueSet>> = None;

    for list in lists {
        let mut this_plain = BTreeSet::new();
        let mut this_keyed: BTreeMap<String, ValueSet> = BTreeMap::new();
        for attr in list {
            match parse_attr(attr) {
                ParsedAttr::Plain(s) => {
                    this_plain.insert(s.to_string());
                }
                ParsedAttr::Keyed(key, values) => {
                    this_keyed.insert(key.to_string(), values.into_owned());
                }
            }
        }

        plain = Some(match plain {
            None => this_plain,
            Some(prev) => prev.intersection(&this_plain).cloned().collect(),
        });

        keyed = Some(match keyed {
            None => this_keyed,
            Some(prev) => prev
                .into_iter()
                .filter_map(|(k, v)| this_keyed.get(&k).map(|other| (k, ValueSet::intersect(v, other.clone()))))
                .collect(),
        });
    }

    let mut out: Vec<String> = plain.unwrap_or_default().into_iter().collect();
    for (key, values) in keyed.unwrap_or_default() {
        if let Some(rendered) = values.render(&key) {
            out.push(rendered);
        }
    }
    out.sort();
    out
}

enum ParsedAttr<'a> {
    Plain(&'a str),
    Keyed(&'a str, BorrowedValueSet<'a>),
}

enum BorrowedValueSet<'a> {
    Wildcard,
    List(Vec<&'a str>),
}

impl BorrowedValueSet<'_> {
    fn into_owned(self) -> ValueSet {
        match self {
            BorrowedValueSet::Wildcard => ValueSet::Wildcard,
            BorrowedValueSet::List(list) => ValueSet::List(list.into_iter().map(String::from).collect()),
        }
    }
}

/// Splits `Key=Value` / `Key=(a, b)` / `Key=*` into a key and its value set;
/// anything else (plain flags, `Key=Ident(args)`) is left opaque.
fn parse_attr(attr: &str) -> ParsedAttr<'_> {
    if let Some(eq) = attr.find('=') {
        let key = &attr[..eq];
        let value = &attr[eq + 1..];
        let key_is_ident = !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if key_is_ident {
            if value == "*" {
                return ParsedAttr::Keyed(key, BorrowedValueSet::Wildcard);
            }
            if let Some(inner) = value.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
                let list = inner.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
                return ParsedAttr::Keyed(key, BorrowedValueSet::List(list));
            }
            if !value.contains('(') {
                return ParsedAttr::Keyed(key, BorrowedValueSet::List(vec![value]));
            }
        }
    }
    ParsedAttr::Plain(attr)
}

#[derive(Clone)]
enum ValueSet {
    /// `Key=*` -- matches anything, so it's the identity element of
    /// intersection.
    Wildcard,
    List(BTreeSet<String>),
}

impl ValueSet {
    fn intersect(a: ValueSet, b: ValueSet) -> ValueSet {
        match (a, b) {
            (ValueSet::Wildcard, ValueSet::Wildcard) => ValueSet::Wildcard,
            (ValueSet::Wildcard, other) | (other, ValueSet::Wildcard) => other,
            (ValueSet::List(a), ValueSet::List(b)) => ValueSet::List(a.intersection(&b).cloned().collect()),
        }
    }

    /// `None` means the key drops out entirely (its value set became empty).
    fn render(&self, key: &str) -> Option<String> {
        match self {
            ValueSet::Wildcard => Some(format!("{key}=*")),
            ValueSet::List(set) => match set.len() {
                0 => None,
                1 => Some(format!("{key}={}", set.iter().next().unwrap())),
                _ => Some(format!("{key}=({})", set.iter().cloned().collect::<Vec<_>>().join(", "))),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MemberKind;

    fn interface(name: &str, attrs: &[&str], members: Vec<Member>) -> Definition {
        Definition::Interface {
            name: name.to_string(),
            inherits: None,
            extended_attributes: attrs.iter().map(|s| s.to_string()).collect(),
            members,
        }
    }

    fn op(name: &str, arg_types: &[&str], modifiers: &[&str]) -> Member {
        Member {
            kind: MemberKind::Operation,
            name: name.to_string(),
            type_: Some("undefined".to_string()),
            arguments: arg_types
                .iter()
                .enumerate()
                .map(|(i, t)| Argument {
                    name: format!("a{i}"),
                    type_: t.to_string(),
                    optional: false,
                    variadic: false,
                    default: None,
                    extended_attributes: Vec::new(),
                })
                .collect(),
            modifiers: modifiers.iter().map(|s| s.to_string()).collect(),
            value: None,
            extended_attributes: Vec::new(),
        }
    }

    fn resolved(defs: Vec<Definition>) -> BTreeMap<String, Definition> {
        defs.into_iter().map(|d| (d.name().to_string(), d)).collect()
    }

    #[test]
    fn overload_kept_only_when_present_in_all_inputs() {
        let a = resolved(vec![interface(
            "Foo",
            &[],
            vec![op("foo", &["long"], &[]), op("foo", &["DOMString"], &[])],
        )]);
        let b = resolved(vec![interface("Foo", &[], vec![op("foo", &["long"], &[])])]);

        let merged = merge_snapshots(&[a, b]);
        assert_eq!(merged.len(), 1);
        let Definition::Interface { members, .. } = &merged[0] else { panic!() };
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].arguments[0].type_, "long");
    }

    #[test]
    fn anonymous_getter_and_setter_coexist() {
        let getter = op("", &["unsigned long"], &["getter"]);
        let setter = op("", &["unsigned long", "any"], &["setter"]);
        let a = resolved(vec![interface("Foo", &[], vec![getter.clone(), setter.clone()])]);
        let b = resolved(vec![interface("Foo", &[], vec![getter, setter])]);

        let merged = merge_snapshots(&[a, b]);
        let Definition::Interface { members, .. } = &merged[0] else { panic!() };
        assert_eq!(members.len(), 2);
    }

    #[test]
    fn single_and_double_iterable_are_distinct() {
        let single = Member {
            kind: MemberKind::Iterable,
            name: String::new(),
            type_: Some("iterable<V>".to_string()),
            arguments: Vec::new(),
            modifiers: Vec::new(),
            value: None,
            extended_attributes: Vec::new(),
        };
        let double = Member {
            type_: Some("iterable<K, V>".to_string()),
            ..single.clone()
        };
        let a = resolved(vec![interface("Foo", &[], vec![single.clone()])]);
        let b = resolved(vec![interface("Foo", &[], vec![double])]);

        let merged = merge_snapshots(&[a, b]);
        // Neither iterable form is common to both inputs.
        let Definition::Interface { members, .. } = &merged[0] else { panic!() };
        assert!(members.is_empty());
    }

    #[test]
    fn exposed_value_lists_intersect_structurally() {
        let attrs = intersect_extended_attributes(&[
            vec!["Exposed=(Window, Worker)".to_string()],
            vec!["Exposed=Window".to_string()],
        ]);
        assert_eq!(attrs, vec!["Exposed=Window".to_string()]);
    }

    #[test]
    fn exposed_wildcard_is_the_identity_element() {
        let attrs = intersect_extended_attributes(&[
            vec!["Exposed=*".to_string()],
            vec!["Exposed=(Window, Worker)".to_string()],
        ]);
        assert_eq!(attrs, vec!["Exposed=(Window, Worker)".to_string()]);
    }

    #[test]
    fn plain_flags_intersect_by_exact_match() {
        let attrs = intersect_extended_attributes(&[
            vec!["SecureContext".to_string(), "LegacyNoInterfaceObject".to_string()],
            vec!["SecureContext".to_string()],
        ]);
        assert_eq!(attrs, vec!["SecureContext".to_string()]);
    }

    #[test]
    fn dictionary_field_required_is_logical_and() {
        let dict = |required: bool| Definition::Dictionary {
            name: "Foo".to_string(),
            inherits: None,
            extended_attributes: Vec::new(),
            fields: vec![Field {
                name: "x".to_string(),
                type_: "long".to_string(),
                required,
                default: None,
                extended_attributes: Vec::new(),
            }],
        };
        let merged = merge_snapshots(&[resolved(vec![dict(true)]), resolved(vec![dict(false)])]);
        let Definition::Dictionary { fields, .. } = &merged[0] else { panic!() };
        assert!(!fields[0].required);
    }

    #[test]
    fn kind_mismatch_drops_the_name() {
        let iface = resolved(vec![interface("Foo", &[], vec![])]);
        let dict = resolved(vec![Definition::Dictionary {
            name: "Foo".to_string(),
            inherits: None,
            extended_attributes: Vec::new(),
            fields: vec![],
        }]);

        let merged = merge_snapshots(&[iface, dict]);
        assert!(merged.is_empty());
    }

    #[test]
    fn empty_interface_is_retained() {
        let a = resolved(vec![interface("Foo", &[], vec![op("bar", &[], &[])])]);
        let b = resolved(vec![interface("Foo", &[], vec![])]);

        let merged = merge_snapshots(&[a, b]);
        assert_eq!(merged.len(), 1);
        let Definition::Interface { members, .. } = &merged[0] else { panic!() };
        assert!(members.is_empty());
    }
}
