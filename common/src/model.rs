//! The normalized, diff-friendly schema we store in `data/objects`.
//!
//! This is deliberately decoupled from weedle's AST: weedle's types borrow
//! from the source string and encode syntax (spans, punctuation tokens) we
//! don't want in a content-addressed diff format. Everything here is owned,
//! sorted where order is not semantically meaningful, and serializes to
//! compact JSON.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Definition {
    Interface {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        inherits: Option<String>,
        extended_attributes: Vec<String>,
        members: Vec<Member>,
    },
    CallbackInterface {
        name: String,
        extended_attributes: Vec<String>,
        members: Vec<Member>,
    },
    Namespace {
        name: String,
        extended_attributes: Vec<String>,
        members: Vec<Member>,
    },
    Dictionary {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        inherits: Option<String>,
        extended_attributes: Vec<String>,
        fields: Vec<Field>,
    },
    Enum {
        name: String,
        extended_attributes: Vec<String>,
        values: Vec<String>,
    },
    Typedef {
        name: String,
        extended_attributes: Vec<String>,
        aliased_type: String,
    },
    Callback {
        name: String,
        extended_attributes: Vec<String>,
        return_type: String,
        arguments: Vec<Argument>,
    },
}

impl Definition {
    pub fn name(&self) -> &str {
        match self {
            Definition::Interface { name, .. }
            | Definition::CallbackInterface { name, .. }
            | Definition::Namespace { name, .. }
            | Definition::Dictionary { name, .. }
            | Definition::Enum { name, .. }
            | Definition::Typedef { name, .. }
            | Definition::Callback { name, .. } => name,
        }
    }

    pub fn name_mut(&mut self) -> &mut String {
        match self {
            Definition::Interface { name, .. }
            | Definition::CallbackInterface { name, .. }
            | Definition::Namespace { name, .. }
            | Definition::Dictionary { name, .. }
            | Definition::Enum { name, .. }
            | Definition::Typedef { name, .. }
            | Definition::Callback { name, .. } => name,
        }
    }

    pub fn extended_attributes_mut(&mut self) -> &mut Vec<String> {
        match self {
            Definition::Interface { extended_attributes, .. }
            | Definition::CallbackInterface { extended_attributes, .. }
            | Definition::Namespace { extended_attributes, .. }
            | Definition::Dictionary { extended_attributes, .. }
            | Definition::Enum { extended_attributes, .. }
            | Definition::Typedef { extended_attributes, .. }
            | Definition::Callback { extended_attributes, .. } => extended_attributes,
        }
    }

    /// Sorts the internal collections (members/fields/values) so that two
    /// definitions built from the same fragments in a different file order
    /// hash identically.
    pub fn canonicalize(&mut self) {
        match self {
            Definition::Interface {
                members,
                extended_attributes,
                ..
            }
            | Definition::CallbackInterface {
                members,
                extended_attributes,
                ..
            }
            | Definition::Namespace {
                members,
                extended_attributes,
                ..
            } => {
                members.sort();
                extended_attributes.sort();
                extended_attributes.dedup();
            }
            Definition::Dictionary {
                fields,
                extended_attributes,
                ..
            } => {
                fields.sort();
                extended_attributes.sort();
                extended_attributes.dedup();
            }
            Definition::Enum {
                values,
                extended_attributes,
                ..
            } => {
                values.sort();
                extended_attributes.sort();
                extended_attributes.dedup();
            }
            Definition::Typedef {
                extended_attributes, ..
            }
            | Definition::Callback {
                extended_attributes, ..
            } => {
                extended_attributes.sort();
                extended_attributes.dedup();
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MemberKind {
    Const,
    Attribute,
    Constructor,
    Operation,
    Iterable,
    AsyncIterable,
    Maplike,
    Setlike,
    Stringifier,
}

/// One member of an interface, callback interface, namespace, or mixin
/// (mixin members are folded into the including interface, see `parse.rs`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Member {
    pub kind: MemberKind,
    /// Empty for unnamed operations (e.g. anonymous getters/setters) and for
    /// iterable/maplike/setlike/stringifier members, which have no name.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    pub arguments: Vec<Argument>,
    pub modifiers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub extended_attributes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Field {
    pub name: String,
    pub type_: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    pub extended_attributes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Argument {
    pub name: String,
    pub type_: String,
    pub optional: bool,
    pub variadic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    pub extended_attributes: Vec<String>,
}
