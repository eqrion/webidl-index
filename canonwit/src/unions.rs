//! WebIDL unions are anonymous; WIT variants must be named. We synthesize a
//! name from the member types (`string-or-s32`) and dedupe structurally
//! identical unions so `(DOMString or long)` only produces one `variant`
//! no matter how many operations reference it.

use std::collections::HashMap;

use wit_encoder::{Type, TypeDef, VariantCase};

use crate::names::NameRegistry;
use crate::report::Report;

/// A human/WIT-legal label for a type, used both for variant case names and
/// for synthesizing a union's own name. Not meant to be a full type-string
/// round-trip -- just distinct and readable.
pub fn type_label(ty: &Type) -> String {
    match ty {
        Type::Bool => "bool".to_string(),
        Type::U8 => "u8".to_string(),
        Type::U16 => "u16".to_string(),
        Type::U32 => "u32".to_string(),
        Type::U64 => "u64".to_string(),
        Type::S8 => "s8".to_string(),
        Type::S16 => "s16".to_string(),
        Type::S32 => "s32".to_string(),
        Type::S64 => "s64".to_string(),
        Type::F32 => "f32".to_string(),
        Type::F64 => "f64".to_string(),
        Type::Char => "char".to_string(),
        Type::String => "string".to_string(),
        Type::Named(id) => id.raw_name().to_string(),
        Type::Borrow(id) => id.raw_name().to_string(),
        Type::Option(inner) => format!("{}-option", type_label(inner)),
        Type::List(inner) => format!("{}-list", type_label(inner)),
        Type::FixedLengthList(inner, n) => format!("{}-list{n}", type_label(inner)),
        Type::Tuple(t) => t.types().iter().map(type_label).collect::<Vec<_>>().join("-and-"),
        Type::Future(Some(inner)) => format!("{}-future", type_label(inner)),
        Type::Future(None) => "future".to_string(),
        Type::Stream(Some(inner)) => format!("{}-stream", type_label(inner)),
        Type::Stream(None) => "stream".to_string(),
        Type::Map(k, v) => format!("{}-{}-map", type_label(k), type_label(v)),
        Type::Result(_) | Type::ErrorContext => "value".to_string(),
    }
}

/// Disambiguates labels that collide within one union (e.g. `DOMString or
/// USVString` both label as `string`) by suffixing `-2`, `-3`, ... This is
/// local to the union, independent of the top-level `NameRegistry`.
fn dedupe_labels(labels: Vec<String>) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    labels
        .into_iter()
        .map(|label| {
            let count = seen.entry(label.clone()).or_insert(0);
            *count += 1;
            if *count == 1 {
                label
            } else {
                format!("{label}-{count}")
            }
        })
        .collect()
}

struct Entry {
    name: String,
    /// (case label, case type), already deduped within this union.
    members: Vec<(String, Type)>,
}

#[derive(Default)]
pub struct UnionRegistry {
    /// structural key (sorted member labels, joined) -> index into `order`.
    by_key: HashMap<String, usize>,
    order: Vec<Entry>,
}

impl UnionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns a union's member set, returning the `Type::named` reference
    /// to use at the call site. Structurally identical member sets (same
    /// labels, any order) share one synthesized variant.
    pub fn intern(&mut self, members: Vec<Type>, names: &mut NameRegistry, report: &mut Report) -> Type {
        let raw_labels: Vec<String> = members.iter().map(type_label).collect();
        let labels = dedupe_labels(raw_labels);
        let mut sorted_labels = labels.clone();
        sorted_labels.sort();
        let key = sorted_labels.join("\0");

        if let Some(&idx) = self.by_key.get(&key) {
            let name = self.order[idx].name.clone();
            report.record_union(&name, &labels);
            return Type::named(name);
        }

        let origin = labels.join("-or-");
        let name = names.register(&origin, report);
        report.record_union(&name, &labels);
        self.by_key.insert(key, self.order.len());
        self.order.push(Entry {
            name: name.clone(),
            members: labels.into_iter().zip(members).collect(),
        });
        Type::named(name)
    }

    /// Emits one `variant` `TypeDef` per unique union, in first-seen order.
    /// Case names are re-escaped here (not when the label was first computed)
    /// so the human-readable label -- several scalar labels (`bool`,
    /// `string`, `list`, ...) are themselves WIT keywords -- stays legible
    /// in the report while the rendered `.wit` stays valid.
    pub fn emit(&self) -> Vec<TypeDef> {
        self.order
            .iter()
            .map(|entry| {
                let cases: Vec<VariantCase> = entry
                    .members
                    .iter()
                    .map(|(label, ty)| VariantCase::value(crate::names::to_kebab(label), ty.clone()))
                    .collect();
                TypeDef::variant(entry.name.clone(), cases)
            })
            .collect()
    }
}
