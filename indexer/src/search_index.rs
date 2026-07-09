//! A compact per-snapshot projection used only by the frontend's search and
//! "explore from a global" navigation -- one file per snapshot instead of
//! requiring the browser to fetch every individual object just to search
//! within members or resolve a member's type to another definition.

use serde::Serialize;

use crate::model::Definition;

#[derive(Serialize)]
pub struct Entry {
    pub name: String,
    pub kind: &'static str,
    pub extended_attributes: Vec<String>,
    pub children: Vec<Child>,
}

#[derive(Serialize)]
pub struct Child {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
}

pub fn build(definitions: &[Definition]) -> Vec<Entry> {
    definitions.iter().map(build_entry).collect()
}

fn build_entry(def: &Definition) -> Entry {
    match def {
        Definition::Interface {
            name,
            extended_attributes,
            members,
            ..
        }
        | Definition::CallbackInterface {
            name,
            extended_attributes,
            members,
            ..
        }
        | Definition::Namespace {
            name,
            extended_attributes,
            members,
            ..
        } => Entry {
            name: name.clone(),
            kind: kind_str(def),
            extended_attributes: extended_attributes.clone(),
            children: members
                .iter()
                .map(|m| Child {
                    name: m.name.clone(),
                    type_: m.type_.clone().unwrap_or_default(),
                })
                .collect(),
        },
        Definition::Dictionary {
            name,
            extended_attributes,
            fields,
            ..
        } => Entry {
            name: name.clone(),
            kind: "dictionary",
            extended_attributes: extended_attributes.clone(),
            children: fields
                .iter()
                .map(|f| Child {
                    name: f.name.clone(),
                    type_: f.type_.clone(),
                })
                .collect(),
        },
        Definition::Enum {
            name,
            extended_attributes,
            values,
        } => Entry {
            name: name.clone(),
            kind: "enum",
            extended_attributes: extended_attributes.clone(),
            children: values
                .iter()
                .map(|v| Child {
                    name: v.clone(),
                    type_: String::new(),
                })
                .collect(),
        },
        Definition::Typedef {
            name,
            extended_attributes,
            aliased_type,
        } => Entry {
            name: name.clone(),
            kind: "typedef",
            extended_attributes: extended_attributes.clone(),
            children: vec![Child {
                name: String::new(),
                type_: aliased_type.clone(),
            }],
        },
        Definition::Callback {
            name,
            extended_attributes,
            return_type,
            arguments,
        } => Entry {
            name: name.clone(),
            kind: "callback",
            extended_attributes: extended_attributes.clone(),
            children: arguments
                .iter()
                .map(|a| Child {
                    name: a.name.clone(),
                    type_: a.type_.clone(),
                })
                .chain(std::iter::once(Child {
                    name: String::new(),
                    type_: return_type.clone(),
                }))
                .collect(),
        },
    }
}

fn kind_str(def: &Definition) -> &'static str {
    match def {
        Definition::Interface { .. } => "interface",
        Definition::CallbackInterface { .. } => "callback_interface",
        Definition::Namespace { .. } => "namespace",
        Definition::Dictionary { .. } => "dictionary",
        Definition::Enum { .. } => "enum",
        Definition::Typedef { .. } => "typedef",
        Definition::Callback { .. } => "callback",
    }
}
