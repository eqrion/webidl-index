//! Pass 2: lowers each WebIDL `Definition` into `wit_encoder` items.
//!
//! Interface/dictionary inheritance is flattened (WIT resources/records
//! can't inherit); overloaded operations/constructors are coalesced to one
//! function, folding shorter overloads into trailing `option<T>` params
//! where they nest as a prefix and dropping the rest; attributes become
//! `get-`/`set-` methods; namespaces become kebab-prefixed free functions.

use std::collections::HashMap;

use common::model::{Definition, Field, Member, MemberKind};
use wit_encoder::{Field as WitField, Interface, Params, ResourceFunc, StandaloneFunc, Type as WitType, TypeDef};

use crate::names::{getter_name, setter_name, to_kebab, NameRegistry};
use crate::report::Report;
use crate::symbols::{SymbolTable, OBJECT};
use crate::typing::{as_param_type, map_attributed_type_string, map_return_type_string, TypeCtx};
use crate::unions::UnionRegistry;

pub fn lower_all(
    defs: &[Definition],
    symbols: &SymbolTable,
    names: &mut NameRegistry,
    iface: &mut Interface,
    report: &mut Report,
) {
    emit_builtins(iface);

    let dict_fields: HashMap<&str, &[Field]> = defs
        .iter()
        .filter_map(|d| match d {
            Definition::Dictionary { name, fields, .. } => Some((name.as_str(), fields.as_slice())),
            _ => None,
        })
        .collect();
    let iface_members: HashMap<&str, &[Member]> = defs
        .iter()
        .filter_map(|d| match d {
            Definition::Interface { name, members, .. } => Some((name.as_str(), members.as_slice())),
            _ => None,
        })
        .collect();

    let mut unions = UnionRegistry::new();

    for def in defs {
        match def {
            Definition::Dictionary { name, fields, .. } => {
                let td = lower_dictionary(name, fields, symbols, &dict_fields, names, &mut unions, report);
                iface.type_def(td);
            }
            Definition::Enum { name, values, .. } => {
                iface.type_def(lower_enum(name, values, symbols, report));
            }
            Definition::Typedef { name, aliased_type, .. } => {
                let td = lower_typedef(name, aliased_type, symbols, names, &mut unions, report);
                iface.type_def(td);
            }
            Definition::Interface { name, members, .. } => {
                let td = lower_interface(name, members, symbols, &iface_members, names, &mut unions, report);
                iface.type_def(td);
            }
            Definition::Namespace { name, members, .. } => {
                lower_namespace(name, members, symbols, names, &mut unions, report, iface);
            }
            Definition::CallbackInterface { .. } | Definition::Callback { .. } => {
                // Only ever referenced as the opaque `object` resource (see
                // `typing::resolve_identifier`); no WIT item of their own.
            }
        }
    }

    for variant_def in unions.emit() {
        iface.type_def(variant_def);
    }
}

fn emit_builtins(iface: &mut Interface) {
    for name in [crate::symbols::ANY, OBJECT, crate::symbols::SYMBOL] {
        iface.type_def(TypeDef::resource(name, Vec::<ResourceFunc>::new()));
    }
}

fn map_or_object(
    s: &str,
    symbols: &SymbolTable,
    names: &mut NameRegistry,
    unions: &mut UnionRegistry,
    report: &mut Report,
    location: String,
) -> WitType {
    let mut ctx = TypeCtx { symbols, names, unions, report, location: location.clone() };
    match map_attributed_type_string(s, &mut ctx) {
        Ok(t) => t,
        Err(_) => {
            report.unknown_type(&location, s);
            WitType::named(OBJECT)
        }
    }
}

fn map_return_or_object(
    s: &str,
    symbols: &SymbolTable,
    names: &mut NameRegistry,
    unions: &mut UnionRegistry,
    report: &mut Report,
    location: String,
) -> crate::typing::ReturnMapping {
    let mut ctx = TypeCtx { symbols, names, unions, report, location: location.clone() };
    match map_return_type_string(s, &mut ctx) {
        Ok(rm) => rm,
        Err(_) => {
            report.unknown_type(&location, s);
            crate::typing::ReturnMapping { is_async: false, result: None }
        }
    }
}

fn lower_dictionary(
    name: &str,
    own_fields: &[Field],
    symbols: &SymbolTable,
    dict_fields: &HashMap<&str, &[Field]>,
    names: &mut NameRegistry,
    unions: &mut UnionRegistry,
    report: &mut Report,
) -> TypeDef {
    let wit_name = symbols.wit_name(name).unwrap_or(name).to_string();

    let mut chain = symbols.ancestors(name);
    chain.reverse();
    let mut all: Vec<&Field> = Vec::new();
    for base in &chain {
        if let Some(fields) = dict_fields.get(*base) {
            all.extend(fields.iter());
            report.flattened(base, &wit_name);
        }
    }
    all.extend(own_fields.iter());

    let mut local = NameRegistry::new();
    let fields: Vec<WitField> = all
        .iter()
        .map(|f| {
            let location = format!("{name}.{}", f.name);
            let mapped = if let Some(target) = symbols.cyclic_ref_in(name, &f.type_) {
                report.cyclic_type(&location, &target);
                WitType::named(OBJECT)
            } else {
                map_or_object(&f.type_, symbols, names, unions, report, location)
            };
            let ty = if f.required || f.default.is_some() { mapped } else { WitType::option(mapped) };
            WitField::new(local.register(&f.name, report), ty)
        })
        .collect();

    TypeDef::record(wit_name, fields)
}

fn lower_enum(name: &str, values: &[String], symbols: &SymbolTable, report: &mut Report) -> TypeDef {
    let wit_name = symbols.wit_name(name).unwrap_or(name).to_string();
    let mut local = NameRegistry::new();
    let cases: Vec<String> = values.iter().map(|v| local.register(v, report)).collect();
    TypeDef::enum_(wit_name, cases)
}

fn lower_typedef(
    name: &str,
    aliased_type: &str,
    symbols: &SymbolTable,
    names: &mut NameRegistry,
    unions: &mut UnionRegistry,
    report: &mut Report,
) -> TypeDef {
    let wit_name = symbols.wit_name(name).unwrap_or(name).to_string();
    let location = format!("typedef {name}");
    let ty = if let Some(target) = symbols.cyclic_ref_in(name, aliased_type) {
        report.cyclic_type(&location, &target);
        WitType::named(OBJECT)
    } else {
        map_or_object(aliased_type, symbols, names, unions, report, location)
    };
    TypeDef::type_(wit_name, ty)
}

/// Picks the arity-maximal overload; shorter overloads whose argument
/// types are a strict prefix of the chosen one's are folded (their extra
/// trailing params become optional); the rest are dropped and reported.
/// Returns the chosen member and the index from which trailing params must
/// be treated as optional (`chosen.arguments.len()` if nothing folded).
fn coalesce_overloads<'a>(
    definition: &str,
    member_label: &str,
    group: &[&'a Member],
    report: &mut Report,
) -> (&'a Member, usize) {
    let chosen = *group.iter().max_by_key(|m| m.arguments.len()).expect("group is non-empty");
    let mut fold_from = chosen.arguments.len();
    let mut dropped = Vec::new();
    for other in group {
        if std::ptr::eq(*other, chosen) {
            continue;
        }
        let is_prefix = other.arguments.len() < chosen.arguments.len()
            && other
                .arguments
                .iter()
                .zip(chosen.arguments.iter())
                .all(|(a, b)| a.type_ == b.type_);
        if is_prefix {
            fold_from = fold_from.min(other.arguments.len());
        } else {
            dropped.push(signature_string(other));
        }
    }
    report.overload(definition, member_label, chosen.arguments.len(), dropped);
    (chosen, fold_from)
}

fn signature_string(m: &Member) -> String {
    let args = m.arguments.iter().map(|a| a.type_.clone()).collect::<Vec<_>>().join(", ");
    format!("{}({args})", m.name)
}

fn build_params(
    m: &Member,
    fold_from: usize,
    symbols: &SymbolTable,
    names: &mut NameRegistry,
    unions: &mut UnionRegistry,
    report: &mut Report,
    location: &str,
) -> Params {
    let mut param_names = NameRegistry::new();
    let items: Vec<(String, WitType)> = m
        .arguments
        .iter()
        .enumerate()
        .map(|(i, arg)| {
            let base = map_or_object(&arg.type_, symbols, names, unions, report, format!("{location}#{}", arg.name));
            let mut ty = if arg.variadic { WitType::list(base) } else { base };
            if !arg.variadic && (arg.optional || i >= fold_from) && !matches!(ty, WitType::Option(_)) {
                ty = WitType::option(ty);
            }
            ty = as_param_type(ty, symbols);
            (param_names.register(&arg.name, report), ty)
        })
        .collect();
    items.into_iter().collect()
}

fn lower_interface(
    name: &str,
    own_members: &[Member],
    symbols: &SymbolTable,
    iface_members: &HashMap<&str, &[Member]>,
    names: &mut NameRegistry,
    unions: &mut UnionRegistry,
    report: &mut Report,
) -> TypeDef {
    let wit_name = symbols.wit_name(name).unwrap_or(name).to_string();

    let mut chain = symbols.ancestors(name);
    chain.reverse();
    let mut all: Vec<&Member> = Vec::new();
    for base in &chain {
        if let Some(members) = iface_members.get(*base) {
            all.extend(members.iter());
            report.flattened(base, &wit_name);
        }
    }
    all.extend(own_members.iter());

    let mut local = NameRegistry::new();
    let mut funcs: Vec<ResourceFunc> = Vec::new();
    let mut omissions: Vec<String> = Vec::new();
    let mut ctor_group: Vec<&Member> = Vec::new();
    let mut op_order: Vec<(String, bool)> = Vec::new();
    let mut op_groups: HashMap<(String, bool), Vec<&Member>> = HashMap::new();

    for m in &all {
        match m.kind {
            MemberKind::Const => omissions.push(report.skip(name, &m.name, "const", "")),
            MemberKind::Iterable | MemberKind::AsyncIterable | MemberKind::Maplike | MemberKind::Setlike => {
                let detail = m.type_.clone().unwrap_or_default();
                omissions.push(report.skip(name, &m.name, "iterable-family", &detail));
            }
            MemberKind::Stringifier => omissions.push(report.skip(name, "(stringifier)", "stringifier", "")),
            MemberKind::Constructor => ctor_group.push(m),
            MemberKind::Attribute => {
                lower_attribute(m, name, symbols, names, unions, report, &mut local, &mut funcs);
            }
            MemberKind::Operation => {
                if m.name.is_empty() {
                    let detail = m.modifiers.join(",");
                    omissions.push(report.skip(name, "(anonymous)", "anonymous-operation", &detail));
                    continue;
                }
                let is_static = m.modifiers.iter().any(|x| x == "static");
                let key = (to_kebab(&m.name), is_static);
                if !op_groups.contains_key(&key) {
                    op_order.push(key.clone());
                }
                op_groups.entry(key).or_default().push(m);
            }
        }
    }

    if !ctor_group.is_empty() {
        let (chosen, fold_from) = coalesce_overloads(name, "constructor", &ctor_group, report);
        let params = build_params(chosen, fold_from, symbols, names, unions, report, &format!("{name}.constructor"));
        let mut ctor = ResourceFunc::constructor();
        ctor.set_params(params);
        funcs.push(ctor);
    }

    for key @ (kebab, is_static) in &op_order {
        let group = &op_groups[key];
        let (chosen, fold_from) = coalesce_overloads(name, kebab, group, report);
        let method_name = local.register(kebab, report);
        let params = build_params(chosen, fold_from, symbols, names, unions, report, &format!("{name}.{}", chosen.name));
        let ret_str = chosen.type_.as_deref().unwrap_or("undefined").to_string();
        let ret = map_return_or_object(&ret_str, symbols, names, unions, report, format!("{name}.{}", chosen.name));
        let mut func = if *is_static {
            ResourceFunc::static_(method_name, ret.is_async)
        } else {
            ResourceFunc::method(method_name, ret.is_async)
        };
        func.set_params(params);
        func.set_result(ret.result);
        funcs.push(func);
    }

    let mut type_def = TypeDef::resource(wit_name, funcs);
    if !omissions.is_empty() {
        type_def.set_docs(Some(omissions.join("\n")));
    }
    type_def
}

#[allow(clippy::too_many_arguments)]
fn lower_attribute(
    m: &Member,
    definition: &str,
    symbols: &SymbolTable,
    names: &mut NameRegistry,
    unions: &mut UnionRegistry,
    report: &mut Report,
    local: &mut NameRegistry,
    funcs: &mut Vec<ResourceFunc>,
) {
    let is_static = m.modifiers.iter().any(|x| x == "static");
    let readonly = m.modifiers.iter().any(|x| x == "readonly");
    let ty_str = m.type_.clone().unwrap_or_default();
    let ty = map_or_object(&ty_str, symbols, names, unions, report, format!("{definition}.{}", m.name));

    let getter_kebab = getter_name(&m.name);
    let mut getter = if is_static {
        ResourceFunc::static_(local.register(&getter_kebab, report), false)
    } else {
        ResourceFunc::method(local.register(&getter_kebab, report), false)
    };
    getter.set_result(Some(ty.clone()));
    funcs.push(getter);

    if !readonly {
        let setter_kebab = setter_name(&m.name);
        let mut setter = if is_static {
            ResourceFunc::static_(local.register(&setter_kebab, report), false)
        } else {
            ResourceFunc::method(local.register(&setter_kebab, report), false)
        };
        let mut params = Params::empty();
        params.push("value", as_param_type(ty, symbols));
        setter.set_params(params);
        funcs.push(setter);
    }
}

fn lower_namespace(
    name: &str,
    members: &[Member],
    symbols: &SymbolTable,
    names: &mut NameRegistry,
    unions: &mut UnionRegistry,
    report: &mut Report,
    iface: &mut Interface,
) {
    let ns_kebab = to_kebab(name);
    let mut op_order: Vec<String> = Vec::new();
    let mut op_groups: HashMap<String, Vec<&Member>> = HashMap::new();

    for m in members {
        match m.kind {
            MemberKind::Operation if !m.name.is_empty() => {
                let kebab = to_kebab(&m.name);
                if !op_groups.contains_key(&kebab) {
                    op_order.push(kebab.clone());
                }
                op_groups.entry(kebab).or_default().push(m);
            }
            MemberKind::Attribute => {
                lower_namespace_attribute(m, name, &ns_kebab, symbols, names, unions, report, iface);
            }
            MemberKind::Const => {
                report.skip(name, &m.name, "const", "");
            }
            _ => {
                report.skip(name, &m.name, "unsupported-namespace-member", &format!("{:?}", m.kind));
            }
        }
    }

    for kebab in &op_order {
        let group = &op_groups[kebab];
        let (chosen, fold_from) = coalesce_overloads(name, kebab, group, report);
        let fname = names.register(&format!("{ns_kebab}-{kebab}"), report);
        let params = build_params(chosen, fold_from, symbols, names, unions, report, &format!("{name}.{}", chosen.name));
        let ret_str = chosen.type_.as_deref().unwrap_or("undefined").to_string();
        let ret = map_return_or_object(&ret_str, symbols, names, unions, report, format!("{name}.{}", chosen.name));
        let mut func = StandaloneFunc::new(fname, ret.is_async);
        func.set_params(params);
        func.set_result(ret.result);
        iface.function(func);
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_namespace_attribute(
    m: &Member,
    definition: &str,
    ns_kebab: &str,
    symbols: &SymbolTable,
    names: &mut NameRegistry,
    unions: &mut UnionRegistry,
    report: &mut Report,
    iface: &mut Interface,
) {
    let ty_str = m.type_.clone().unwrap_or_default();
    let ty = map_or_object(&ty_str, symbols, names, unions, report, format!("{definition}.{}", m.name));

    let getter_name_str = names.register(&format!("{ns_kebab}-{}", getter_name(&m.name)), report);
    let mut getter = StandaloneFunc::new(getter_name_str, false);
    getter.set_result(Some(ty.clone()));
    iface.function(getter);

    if !m.modifiers.iter().any(|x| x == "readonly") {
        let setter_name_str = names.register(&format!("{ns_kebab}-{}", setter_name(&m.name)), report);
        let mut setter = StandaloneFunc::new(setter_name_str, false);
        let mut params = Params::empty();
        params.push("value", as_param_type(ty, symbols));
        setter.set_params(params);
        iface.function(setter);
    }
}
