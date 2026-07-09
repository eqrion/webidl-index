//! WIT identifiers are kebab-case; WebIDL names are camelCase/PascalCase.
//! `heck` already splits acronym runs the way we want (`XMLHttpRequest` ->
//! `xml-http-request`, `Uint8Array` -> `uint8-array`); we only add the
//! WIT-specific bits heck doesn't know about: a leading-digit escape and
//! per-scope collision avoidance.

use std::collections::HashMap;

use heck::AsKebabCase;

use crate::report::Report;

/// The WIT lexer's full keyword list (`wit-parser`'s `ast/lex.rs`).
/// `wit_encoder::Ident`'s own `%`-escaping list is missing `map` and `_`
/// relative to this, so we escape ourselves rather than rely on it.
const WIT_KEYWORDS: &[&str] = &[
    "use", "type", "func", "u8", "u16", "u32", "u64", "s8", "s16", "s32", "s64", "f32", "f64",
    "char", "resource", "own", "borrow", "record", "flags", "variant", "enum", "bool", "string",
    "option", "result", "future", "stream", "error-context", "list", "map", "_", "as", "from",
    "static", "interface", "tuple", "world", "import", "export", "package", "constructor",
    "include", "with", "async",
];

/// Converts a WebIDL identifier to a WIT-legal kebab-case identifier,
/// `%`-escaping it if it collides with a WIT keyword.
pub fn to_kebab(name: &str) -> String {
    let kebab = AsKebabCase(name).to_string();
    let kebab = if kebab.is_empty() { "unnamed".to_string() } else { kebab };
    let kebab = if kebab.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("x{kebab}")
    } else {
        kebab
    };
    if WIT_KEYWORDS.contains(&kebab.as_str()) {
        format!("%{kebab}")
    } else {
        kebab
    }
}

pub fn getter_name(attr_name: &str) -> String {
    format!("get-{}", to_kebab(attr_name))
}

pub fn setter_name(attr_name: &str) -> String {
    format!("set-{}", to_kebab(attr_name))
}

/// Tracks which kebab-case names are already taken within one WIT scope
/// (an interface's top-level items, or one resource's methods), so two
/// WebIDL names that mangle to the same kebab string don't collide silently.
#[derive(Default)]
pub struct NameRegistry {
    /// kebab name -> the WebIDL name that first claimed it.
    taken: HashMap<String, String>,
}

impl NameRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `origin`'s mangled name in this scope, appending `-2`,
    /// `-3`, ... on collision. Reports every rename.
    pub fn register(&mut self, origin: &str, report: &mut Report) -> String {
        let base = to_kebab(origin);
        if !self.taken.contains_key(&base) {
            self.taken.insert(base.clone(), origin.to_string());
            return base;
        }
        let mut n = 2;
        loop {
            let candidate = format!("{base}-{n}");
            if !self.taken.contains_key(&candidate) {
                self.taken.insert(candidate.clone(), origin.to_string());
                report.renamed(origin, &candidate);
                return candidate;
            }
            n += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_cases() {
        assert_eq!(to_kebab("HTMLElement"), "html-element");
        assert_eq!(to_kebab("innerHTML"), "inner-html");
        assert_eq!(to_kebab("XMLHttpRequest"), "xml-http-request");
        assert_eq!(to_kebab("getElementById"), "get-element-by-id");
        assert_eq!(to_kebab("toJSON"), "to-json");
        assert_eq!(to_kebab("Uint8Array"), "uint8-array");
        assert_eq!(to_kebab("RTCPeerConnection"), "rtc-peer-connection");
        assert_eq!(to_kebab("URL"), "url");
        assert_eq!(to_kebab("2dContext"), "x2d-context");
        assert_eq!(to_kebab(""), "unnamed");
    }

    #[test]
    fn accessor_names() {
        assert_eq!(getter_name("innerHTML"), "get-inner-html");
        assert_eq!(setter_name("innerHTML"), "set-inner-html");
    }

    #[test]
    fn registry_dedupes_collisions() {
        let mut report = Report::default();
        let mut reg = NameRegistry::new();
        assert_eq!(reg.register("HtmlElement", &mut report), "html-element");
        assert_eq!(reg.register("HTMLElement", &mut report), "html-element-2");
        assert_eq!(report.renames.len(), 1);
    }
}
