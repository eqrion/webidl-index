//! Parses a set of WebIDL files with `weedle` and merges them into one
//! `model::Definition` per name.
//!
//! WebIDL spreads a single interface's real API surface across a main
//! definition, `partial interface` blocks, and `interface mixin`s pulled in
//! via `X includes Y;` -- often in different files. We fold all of that
//! together here so a snapshot has exactly one entry per name that reflects
//! what the API actually looks like, not how the source happened to be split.

use std::collections::BTreeMap;

use weedle::Parse;

use crate::model::{Argument, Definition, Field, Member, MemberKind};
use crate::render;

#[derive(Default)]
struct InterfaceAcc {
    inherits: Option<String>,
    extended_attributes: Vec<String>,
    members: Vec<Member>,
}

#[derive(Default)]
struct DictAcc {
    inherits: Option<String>,
    extended_attributes: Vec<String>,
    fields: Vec<Field>,
}

#[derive(Default)]
struct NamespaceAcc {
    extended_attributes: Vec<String>,
    members: Vec<Member>,
}

#[derive(Default)]
struct CallbackInterfaceAcc {
    extended_attributes: Vec<String>,
    members: Vec<Member>,
}

pub struct ParseError {
    pub file: String,
    pub message: String,
}

pub struct MergeResult {
    pub definitions: Vec<Definition>,
    pub errors: Vec<ParseError>,
}

/// Rewrites source text to work around gaps between weedle's grammar and
/// what browsers actually check in, before handing the result to weedle.
/// Each rewrite is narrow and was confirmed necessary by tracing real parse
/// failures against Firefox's `dom/webidl` (see the comment on each regex):
///
/// - Gecko forward-declares external XPCOM/C++ types with `interface Foo;`
///   (no body) so they can be referenced as parameter/return types. Valid in
///   Gecko's own IDL preprocessor, not in the WebIDL spec grammar weedle
///   implements, and these carry no member information we'd index anyway.
/// - `#ifdef`/`#ifndef`/`#endif` C-preprocessor conditionals gate a handful
///   of build-specific members. We strip only the directive lines and keep
///   every guarded body, i.e. we index the union of all build configs (an
///   accepted approximation; no `#else` branches exist in the corpus today,
///   so this never doubles up conflicting content).
/// - `sequence<[ExtAttr] Type>` (extended attributes on a generic's type
///   argument) is valid per the WebIDL spec's `TypeWithExtendedAttributes`
///   production, but weedle's `SequenceType`/`FrozenArrayType`/
///   `ObservableArrayType` hardcode a plain `Type` -- confirmed the same gap
///   exists in the `weedle4` fork, so this isn't a weedle-specific bug to
///   route around by switching crates. We drop the attribute list; it
///   refines binding behavior only, not the type identity we diff on.
/// - `callback constructor Foo = Type(args);` (a constructible callback,
///   used by Custom Elements and Worklets) isn't in weedle's `Definition`
///   grammar at all, in either fork. We drop the `constructor` keyword and
///   index it as a plain callback, losing only the "invoked with `new`"
///   marker -- the signature we actually diff on is unaffected.
/// - Blink's own binding-generator extended attributes allow a quoted-string
///   list, e.g. `ReflectOnly=("on","off")`. weedle's `ExtendedAttributeIdentList`
///   only allows bare identifiers in that position. We strip the quotes so
///   the list parses as identifiers; the value we render is identical either
///   way (`render::ident_or_string` doesn't distinguish them further).
/// - WebKit's `.idl` style commonly leaves a trailing comma before `]` in
///   multi-line extended attribute lists, e.g. `[\n  Foo,\n]`. weedle's
///   `ExtendedAttributeList` (unlike its `EnumValueList`, which explicitly
///   allows a trailing separator) doesn't permit one. `[...]` is used for
///   nothing else in the grammar, so stripping any comma directly before a
///   `]` is unambiguous. Confirmed as the dominant failure mode across
///   WebKit's corpus (~460 of ~1700 files) before this fix.
/// - WebKit spells "both conditions must hold" as `Conditional=A&B`, but `&`
///   isn't a valid `Identifier` character, so weedle rejects the whole
///   value. We merge it into one identifier (`A_B`); we only need the value
///   to round-trip as an opaque string, not to evaluate the condition.
/// - The WebIDL spec's async iterable declaration is `async_iterable<...>`
///   (one token), but weedle 0.13.1 only implements the older two-token
///   `async iterable<...>` spelling. We rewrite to the older spelling; the
///   declaration shape (single- or double-typed, with or without arguments)
///   is unaffected.
/// - HTML spells validation hints on reflected attributes as
///   `ReflectDefault=1`, `ReflectDefault=1.0`, or `ReflectRange=(0, 8)` -- an
///   `Identifier` value is the only kind weedle's
///   `ExtendedAttributeIdent`/`IdentList` accept, and numbers aren't
///   identifiers. We drop these attributes entirely (plus one adjacent
///   comma, to keep the surrounding list well-formed); they refine
///   attribute-reflection behavior only, not the type identity we diff on.
fn preprocess(content: &str) -> std::borrow::Cow<'_, str> {
    static FORWARD_DECL: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static PREPROCESSOR_DIRECTIVE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static GENERIC_EXT_ATTR: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static CALLBACK_CONSTRUCTOR: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static QUOTED_IDENT_LIST: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static TRAILING_COMMA: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static IDENT_AMPERSAND: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static ASYNC_ITERABLE_TOKEN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static NUMERIC_EXT_ATTR_VALUE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

    let forward_decl = FORWARD_DECL.get_or_init(|| {
        regex::Regex::new(r"(?m)^[ \t]*interface[ \t]+[A-Za-z_][\w-]*[ \t]*;[ \t]*$").unwrap()
    });
    let preprocessor_directive =
        PREPROCESSOR_DIRECTIVE.get_or_init(|| regex::Regex::new(r"(?m)^[ \t]*#\w.*$").unwrap());
    let generic_ext_attr =
        GENERIC_EXT_ATTR.get_or_init(|| regex::Regex::new(r"<\s*\[[^\[\]]*\]\s*").unwrap());
    let callback_constructor = CALLBACK_CONSTRUCTOR.get_or_init(|| {
        regex::Regex::new(r"(?m)^([ \t]*callback[ \t]+)constructor[ \t]+").unwrap()
    });
    let quoted_ident_list =
        QUOTED_IDENT_LIST.get_or_init(|| regex::Regex::new(r"=\(([^()]*)\)").unwrap());
    let trailing_comma = TRAILING_COMMA.get_or_init(|| regex::Regex::new(r",(\s*)\]").unwrap());
    let ident_ampersand =
        IDENT_AMPERSAND.get_or_init(|| regex::Regex::new(r"(\w)&(\w)").unwrap());
    let async_iterable_token =
        ASYNC_ITERABLE_TOKEN.get_or_init(|| regex::Regex::new(r"\basync_iterable\b").unwrap());
    let numeric_ext_attr_value = NUMERIC_EXT_ATTR_VALUE.get_or_init(|| {
        regex::Regex::new(
            r"(,\s*)?\b[A-Za-z_][\w-]*=(\(-?\d+(?:\.\d+)?(?:,\s*-?\d+(?:\.\d+)?)*\)|-?\d+(?:\.\d+)?)",
        )
        .unwrap()
    });

    let s = forward_decl.replace_all(content, "");
    let s = preprocessor_directive.replace_all(&s, "").into_owned();
    let s = generic_ext_attr.replace_all(&s, "<").into_owned();
    let s = callback_constructor.replace_all(&s, "$1").into_owned();
    let s = ident_ampersand.replace_all(&s, "${1}_$2").into_owned();
    let s = async_iterable_token.replace_all(&s, "async iterable").into_owned();
    let s = numeric_ext_attr_value.replace_all(&s, "").into_owned();
    let s = quoted_ident_list
        .replace_all(&s, |caps: &regex::Captures| {
            // Split, unquote, and drop empty items (e.g. `ReflectOnly=("",
            // "no-referrer")`) individually rather than stripping all `"`
            // chars in one pass -- a blanket strip turns `("",...)` into a
            // leading empty slot, which isn't a valid identifier list.
            let items: Vec<&str> = caps[1]
                .split(',')
                .map(|item| item.trim().trim_matches('"'))
                .filter(|item| !item.is_empty())
                .collect();
            format!("=({})", items.join(","))
        })
        .into_owned();
    trailing_comma.replace_all(&s, "$1]").into_owned().into()
}

/// Parses and merges every file. `files` is `(relative path, contents)`,
/// used only for error attribution.
pub fn merge_files(files: &[(String, String)]) -> MergeResult {
    let mut interfaces: BTreeMap<String, InterfaceAcc> = BTreeMap::new();
    let mut mixins: BTreeMap<String, Vec<Member>> = BTreeMap::new();
    let mut mixin_attrs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut namespaces: BTreeMap<String, NamespaceAcc> = BTreeMap::new();
    let mut dictionaries: BTreeMap<String, DictAcc> = BTreeMap::new();
    let mut enums: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut enum_attrs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut typedefs: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    let mut callbacks: BTreeMap<String, (String, Vec<Argument>, Vec<String>)> = BTreeMap::new();
    let mut callback_interfaces: BTreeMap<String, CallbackInterfaceAcc> = BTreeMap::new();
    let mut includes: Vec<(String, String)> = Vec::new();
    let mut errors = Vec::new();

    for (path, content) in files {
        let content = preprocess(content);

        // Not `weedle::parse`: it panics on trailing unparsed input instead
        // of returning an error, which would take down the whole run over
        // one file weedle's grammar doesn't cover. Calling the lower-level
        // `Definitions::parse` directly lets us keep whatever prefix it did
        // parse and record the rest as a soft, per-file error.
        let (remaining, parsed) = match weedle::Definitions::parse(&content) {
            Ok(r) => r,
            Err(e) => {
                errors.push(ParseError {
                    file: path.clone(),
                    message: format!("{e:?}"),
                });
                continue;
            }
        };
        if !remaining.trim().is_empty() {
            let snippet: String = remaining.chars().take(120).collect();
            errors.push(ParseError {
                file: path.clone(),
                message: format!("stopped before end of file, near: {snippet:?}"),
            });
        }
        for def in parsed {
            apply_definition(
                def,
                &mut interfaces,
                &mut mixins,
                &mut mixin_attrs,
                &mut namespaces,
                &mut dictionaries,
                &mut enums,
                &mut enum_attrs,
                &mut typedefs,
                &mut callbacks,
                &mut callback_interfaces,
                &mut includes,
            );
        }
    }

    for (lhs, rhs) in &includes {
        if let Some(mixin_members) = mixins.get(rhs) {
            let acc = interfaces.entry(lhs.clone()).or_default();
            acc.members.extend(mixin_members.clone());
        }
        if let Some(attrs) = mixin_attrs.get(rhs) {
            let acc = interfaces.entry(lhs.clone()).or_default();
            acc.extended_attributes.extend(attrs.clone());
        }
        if !mixins.contains_key(rhs) && !mixin_attrs.contains_key(rhs) {
            errors.push(ParseError {
                file: format!("{lhs} includes {rhs}"),
                message: "includes target mixin was never defined".to_string(),
            });
        }
    }

    let mut definitions = Vec::new();
    for (name, acc) in interfaces {
        definitions.push(Definition::Interface {
            name,
            inherits: acc.inherits,
            extended_attributes: acc.extended_attributes,
            members: acc.members,
        });
    }
    for (name, acc) in callback_interfaces {
        definitions.push(Definition::CallbackInterface {
            name,
            extended_attributes: acc.extended_attributes,
            members: acc.members,
        });
    }
    for (name, acc) in namespaces {
        definitions.push(Definition::Namespace {
            name,
            extended_attributes: acc.extended_attributes,
            members: acc.members,
        });
    }
    for (name, acc) in dictionaries {
        definitions.push(Definition::Dictionary {
            name,
            inherits: acc.inherits,
            extended_attributes: acc.extended_attributes,
            fields: acc.fields,
        });
    }
    for (name, values) in enums {
        let extended_attributes = enum_attrs.remove(&name).unwrap_or_default();
        definitions.push(Definition::Enum {
            name,
            extended_attributes,
            values,
        });
    }
    for (name, (aliased_type, extended_attributes)) in typedefs {
        definitions.push(Definition::Typedef {
            name,
            extended_attributes,
            aliased_type,
        });
    }
    for (name, (return_type, arguments, extended_attributes)) in callbacks {
        definitions.push(Definition::Callback {
            name,
            extended_attributes,
            return_type,
            arguments,
        });
    }

    definitions.sort_by(|a, b| a.name().cmp(b.name()));
    resolve_interface_names(&mut definitions);

    for def in &mut definitions {
        def.canonicalize();
    }

    MergeResult { definitions, errors }
}

/// WebKit's binding generator uses `[InterfaceName=X]` when the IDL/C++
/// identifier differs from the name actually exposed to JS -- almost always
/// to avoid colliding with an existing internal WebCore class of the same
/// name (e.g. the IDL/C++ type is `DOMWindow`, exposed to JS as `Window`).
/// Renames each definition to its `InterfaceName` value when that doesn't
/// collide with another definition's name.
///
/// Processing in (already sorted) name order and tracking claimed names
/// handles the one real collision in the corpus: WebKit's site-isolation
/// split of `Window` into `LocalDOMWindow`/`RemoteDOMWindow` (same- and
/// cross-process backing classes for one JS-visible interface) makes both
/// declare `[InterfaceName=Window]`. `LocalDOMWindow` sorts first and claims
/// `Window`; `RemoteDOMWindow` finds it taken and keeps its own raw name.
/// That's the right outcome, not a fallback to paper over: the two aren't
/// duplicates, `RemoteDOMWindow` is a real, much smaller (~14 vs. ~234
/// members) cross-process-safe subset, so renaming it over `Window` would
/// have silently discarded whichever definition didn't win.
///
/// A mixin folded in via `includes` can carry its own (unrelated)
/// `InterfaceName` on itself, appended to a definition's attribute list
/// after its own by the `includes` loop above -- taking the *first* match
/// keeps the interface's own declaration authoritative.
fn resolve_interface_names(definitions: &mut [Definition]) {
    let mut claimed: std::collections::HashSet<String> =
        definitions.iter().map(|d| d.name().to_string()).collect();

    for def in definitions.iter_mut() {
        let current_name = def.name().to_string();
        let attrs = def.extended_attributes_mut();
        let Some(pos) = attrs.iter().position(|a| a.starts_with("InterfaceName=")) else {
            continue;
        };
        let desired = attrs[pos]["InterfaceName=".len()..].to_string();
        if desired == current_name || claimed.contains(&desired) {
            continue;
        }
        attrs.retain(|a| !a.starts_with("InterfaceName="));
        claimed.remove(&current_name);
        claimed.insert(desired.clone());
        *def.name_mut() = desired;
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_definition<'a>(
    def: weedle::Definition<'a>,
    interfaces: &mut BTreeMap<String, InterfaceAcc>,
    mixins: &mut BTreeMap<String, Vec<Member>>,
    mixin_attrs: &mut BTreeMap<String, Vec<String>>,
    namespaces: &mut BTreeMap<String, NamespaceAcc>,
    dictionaries: &mut BTreeMap<String, DictAcc>,
    enums: &mut BTreeMap<String, Vec<String>>,
    enum_attrs: &mut BTreeMap<String, Vec<String>>,
    typedefs: &mut BTreeMap<String, (String, Vec<String>)>,
    callbacks: &mut BTreeMap<String, (String, Vec<Argument>, Vec<String>)>,
    callback_interfaces: &mut BTreeMap<String, CallbackInterfaceAcc>,
    includes: &mut Vec<(String, String)>,
) {
    use weedle::Definition as D;
    match def {
        D::Interface(i) => {
            let acc = interfaces.entry(i.identifier.0.to_string()).or_default();
            acc.inherits = i.inheritance.map(|inh| inh.identifier.0.to_string());
            acc.extended_attributes
                .extend(render::extended_attributes(&i.attributes));
            acc.members.extend(i.members.body.iter().map(interface_member));
        }
        D::PartialInterface(i) => {
            let acc = interfaces.entry(i.identifier.0.to_string()).or_default();
            acc.extended_attributes
                .extend(render::extended_attributes(&i.attributes));
            acc.members.extend(i.members.body.iter().map(interface_member));
        }
        D::InterfaceMixin(m) => {
            mixins
                .entry(m.identifier.0.to_string())
                .or_default()
                .extend(m.members.body.iter().map(mixin_member));
            mixin_attrs
                .entry(m.identifier.0.to_string())
                .or_default()
                .extend(render::extended_attributes(&m.attributes));
        }
        D::PartialInterfaceMixin(m) => {
            mixins
                .entry(m.identifier.0.to_string())
                .or_default()
                .extend(m.members.body.iter().map(mixin_member));
        }
        D::Namespace(n) => {
            let acc = namespaces.entry(n.identifier.0.to_string()).or_default();
            acc.extended_attributes
                .extend(render::extended_attributes(&n.attributes));
            acc.members.extend(n.members.body.iter().map(namespace_member));
        }
        D::PartialNamespace(n) => {
            let acc = namespaces.entry(n.identifier.0.to_string()).or_default();
            acc.members.extend(n.members.body.iter().map(namespace_member));
        }
        D::Dictionary(dict) => {
            let acc = dictionaries.entry(dict.identifier.0.to_string()).or_default();
            acc.inherits = dict.inheritance.map(|inh| inh.identifier.0.to_string());
            acc.extended_attributes
                .extend(render::extended_attributes(&dict.attributes));
            acc.fields.extend(dict.members.body.iter().map(dictionary_member));
        }
        D::PartialDictionary(dict) => {
            let acc = dictionaries.entry(dict.identifier.0.to_string()).or_default();
            acc.fields.extend(dict.members.body.iter().map(dictionary_member));
        }
        D::Enum(e) => {
            enums
                .entry(e.identifier.0.to_string())
                .or_default()
                .extend(e.values.body.list.iter().map(|v| v.0.to_string()));
            enum_attrs
                .entry(e.identifier.0.to_string())
                .or_default()
                .extend(render::extended_attributes(&e.attributes));
        }
        D::Typedef(t) => {
            typedefs.insert(
                t.identifier.0.to_string(),
                (
                    render::attributed_type(&t.type_),
                    render::extended_attributes(&t.attributes),
                ),
            );
        }
        D::Callback(c) => {
            callbacks.insert(
                c.identifier.0.to_string(),
                (
                    render::return_type(&c.return_type),
                    render::arguments(&c.arguments.body),
                    render::extended_attributes(&c.attributes),
                ),
            );
        }
        D::CallbackInterface(c) => {
            let acc = callback_interfaces
                .entry(c.identifier.0.to_string())
                .or_default();
            acc.extended_attributes
                .extend(render::extended_attributes(&c.attributes));
            acc.members.extend(c.members.body.iter().map(interface_member));
        }
        D::IncludesStatement(s) => {
            includes.push((
                s.lhs_identifier.0.to_string(),
                s.rhs_identifier.0.to_string(),
            ));
        }
        D::Implements(s) => {
            includes.push((
                s.lhs_identifier.0.to_string(),
                s.rhs_identifier.0.to_string(),
            ));
        }
    }
}

fn interface_member(m: &weedle::interface::InterfaceMember<'_>) -> Member {
    use weedle::interface::InterfaceMember as IM;
    match m {
        IM::Const(c) => const_member(c),
        IM::Attribute(a) => {
            let mut modifiers = Vec::new();
            if let Some(m) = &a.modifier {
                modifiers.push(stringifier_or_inherit_or_static(m));
            }
            if a.readonly.is_some() {
                modifiers.push("readonly".to_string());
            }
            Member {
                kind: MemberKind::Attribute,
                name: a.identifier.0.to_string(),
                type_: Some(render::attributed_type(&a.type_)),
                arguments: Vec::new(),
                modifiers,
                value: None,
                extended_attributes: render::extended_attributes(&a.attributes),
            }
        }
        IM::Constructor(c) => Member {
            kind: MemberKind::Constructor,
            name: String::new(),
            type_: None,
            arguments: render::arguments(&c.args.body),
            modifiers: Vec::new(),
            value: None,
            extended_attributes: render::extended_attributes(&c.attributes),
        },
        IM::Operation(o) => {
            let mut modifiers = Vec::new();
            if let Some(m) = &o.modifier {
                modifiers.push(stringifier_or_static(m));
            }
            if let Some(s) = &o.special {
                modifiers.push(special(s));
            }
            Member {
                kind: MemberKind::Operation,
                name: o
                    .identifier
                    .as_ref()
                    .map(|i| i.0.to_string())
                    .unwrap_or_default(),
                type_: Some(render::return_type(&o.return_type)),
                arguments: render::arguments(&o.args.body),
                modifiers,
                value: None,
                extended_attributes: render::extended_attributes(&o.attributes),
            }
        }
        IM::Iterable(it) => iterable_member(it),
        IM::AsyncIterable(it) => async_iterable_member(it),
        IM::Maplike(m) => Member {
            kind: MemberKind::Maplike,
            name: String::new(),
            type_: Some(format!(
                "maplike<{}, {}>",
                render::attributed_type(&m.generics.body.0),
                render::attributed_type(&m.generics.body.2)
            )),
            arguments: Vec::new(),
            modifiers: if m.readonly.is_some() {
                vec!["readonly".to_string()]
            } else {
                Vec::new()
            },
            value: None,
            extended_attributes: render::extended_attributes(&m.attributes),
        },
        IM::Setlike(m) => Member {
            kind: MemberKind::Setlike,
            name: String::new(),
            type_: Some(format!(
                "setlike<{}>",
                render::attributed_type(&m.generics.body)
            )),
            arguments: Vec::new(),
            modifiers: if m.readonly.is_some() {
                vec!["readonly".to_string()]
            } else {
                Vec::new()
            },
            value: None,
            extended_attributes: render::extended_attributes(&m.attributes),
        },
        IM::Stringifier(s) => stringifier_member(s),
    }
}

fn mixin_member(m: &weedle::mixin::MixinMember<'_>) -> Member {
    use weedle::mixin::MixinMember as M;
    match m {
        M::Const(c) => const_member(c),
        M::Operation(o) => {
            let mut modifiers = Vec::new();
            if o.stringifier.is_some() {
                modifiers.push("stringifier".to_string());
            }
            Member {
                kind: MemberKind::Operation,
                name: o
                    .identifier
                    .as_ref()
                    .map(|i| i.0.to_string())
                    .unwrap_or_default(),
                type_: Some(render::return_type(&o.return_type)),
                arguments: render::arguments(&o.args.body),
                modifiers,
                value: None,
                extended_attributes: render::extended_attributes(&o.attributes),
            }
        }
        M::Attribute(a) => {
            let mut modifiers = Vec::new();
            if a.stringifier.is_some() {
                modifiers.push("stringifier".to_string());
            }
            if a.readonly.is_some() {
                modifiers.push("readonly".to_string());
            }
            Member {
                kind: MemberKind::Attribute,
                name: a.identifier.0.to_string(),
                type_: Some(render::attributed_type(&a.type_)),
                arguments: Vec::new(),
                modifiers,
                value: None,
                extended_attributes: render::extended_attributes(&a.attributes),
            }
        }
        M::Stringifier(s) => stringifier_member(s),
    }
}

fn namespace_member(m: &weedle::namespace::NamespaceMember<'_>) -> Member {
    use weedle::namespace::NamespaceMember as N;
    match m {
        N::Const(c) => Member {
            kind: MemberKind::Const,
            name: c.identifier.0.to_string(),
            type_: Some(render::const_type(&c.const_type)),
            arguments: Vec::new(),
            modifiers: Vec::new(),
            value: Some(render::const_value(&c.const_value)),
            extended_attributes: render::extended_attributes(&c.attributes),
        },
        N::Operation(o) => Member {
            kind: MemberKind::Operation,
            name: o
                .identifier
                .as_ref()
                .map(|i| i.0.to_string())
                .unwrap_or_default(),
            type_: Some(render::return_type(&o.return_type)),
            arguments: render::arguments(&o.args.body),
            modifiers: Vec::new(),
            value: None,
            extended_attributes: render::extended_attributes(&o.attributes),
        },
        N::Attribute(a) => Member {
            kind: MemberKind::Attribute,
            name: a.identifier.0.to_string(),
            type_: Some(render::attributed_type(&a.type_)),
            arguments: Vec::new(),
            modifiers: vec!["readonly".to_string()],
            value: None,
            extended_attributes: render::extended_attributes(&a.attributes),
        },
    }
}

fn dictionary_member(m: &weedle::dictionary::DictionaryMember<'_>) -> Field {
    Field {
        name: m.identifier.0.to_string(),
        type_: render::type_(&m.type_),
        required: m.required.is_some(),
        default: render::default_rhs(&m.default),
        extended_attributes: render::extended_attributes(&m.attributes),
    }
}

fn const_member(c: &weedle::interface::ConstMember<'_>) -> Member {
    Member {
        kind: MemberKind::Const,
        name: c.identifier.0.to_string(),
        type_: Some(render::const_type(&c.const_type)),
        arguments: Vec::new(),
        modifiers: Vec::new(),
        value: Some(render::const_value(&c.const_value)),
        extended_attributes: render::extended_attributes(&c.attributes),
    }
}

fn stringifier_member(s: &weedle::interface::StringifierMember<'_>) -> Member {
    Member {
        kind: MemberKind::Stringifier,
        name: String::new(),
        type_: None,
        arguments: Vec::new(),
        modifiers: Vec::new(),
        value: None,
        extended_attributes: render::extended_attributes(&s.attributes),
    }
}

fn iterable_member(it: &weedle::interface::IterableInterfaceMember<'_>) -> Member {
    use weedle::interface::IterableInterfaceMember as I;
    match it {
        I::Single(s) => Member {
            kind: MemberKind::Iterable,
            name: String::new(),
            type_: Some(format!(
                "iterable<{}>",
                render::attributed_type(&s.generics.body)
            )),
            arguments: Vec::new(),
            modifiers: Vec::new(),
            value: None,
            extended_attributes: render::extended_attributes(&s.attributes),
        },
        I::Double(d) => Member {
            kind: MemberKind::Iterable,
            name: String::new(),
            type_: Some(format!(
                "iterable<{}, {}>",
                render::attributed_type(&d.generics.body.0),
                render::attributed_type(&d.generics.body.2)
            )),
            arguments: Vec::new(),
            modifiers: Vec::new(),
            value: None,
            extended_attributes: render::extended_attributes(&d.attributes),
        },
    }
}

fn async_iterable_member(it: &weedle::interface::AsyncIterableInterfaceMember<'_>) -> Member {
    use weedle::interface::AsyncIterableInterfaceMember as A;
    match it {
        A::Single(s) => Member {
            kind: MemberKind::AsyncIterable,
            name: String::new(),
            type_: Some(format!(
                "async iterable<{}>",
                render::attributed_type(&s.generics.body)
            )),
            arguments: s
                .args
                .as_ref()
                .map(|a| render::arguments(&a.body))
                .unwrap_or_default(),
            modifiers: Vec::new(),
            value: None,
            extended_attributes: render::extended_attributes(&s.attributes),
        },
        A::Double(d) => Member {
            kind: MemberKind::AsyncIterable,
            name: String::new(),
            type_: Some(format!(
                "async iterable<{}, {}>",
                render::attributed_type(&d.generics.body.0),
                render::attributed_type(&d.generics.body.2)
            )),
            arguments: d
                .args
                .as_ref()
                .map(|a| render::arguments(&a.body))
                .unwrap_or_default(),
            modifiers: Vec::new(),
            value: None,
            extended_attributes: render::extended_attributes(&d.attributes),
        },
    }
}

fn stringifier_or_inherit_or_static(
    m: &weedle::interface::StringifierOrInheritOrStatic,
) -> String {
    use weedle::interface::StringifierOrInheritOrStatic as S;
    match m {
        S::Stringifier(_) => "stringifier".to_string(),
        S::Inherit(_) => "inherit".to_string(),
        S::Static(_) => "static".to_string(),
    }
}

fn stringifier_or_static(m: &weedle::interface::StringifierOrStatic) -> String {
    use weedle::interface::StringifierOrStatic as S;
    match m {
        S::Stringifier(_) => "stringifier".to_string(),
        S::Static(_) => "static".to_string(),
    }
}

fn special(s: &weedle::interface::Special) -> String {
    use weedle::interface::Special as S;
    match s {
        S::Getter(_) => "getter".to_string(),
        S::Setter(_) => "setter".to_string(),
        S::Deleter(_) => "deleter".to_string(),
        S::LegacyCaller(_) => "legacycaller".to_string(),
    }
}
