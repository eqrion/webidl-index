//! Every heuristic decision the converter makes that loses or reshapes
//! information gets recorded here, then surfaced three ways: a `///` doc
//! comment right at the spot in the emitted `.wit` file, a stderr summary,
//! and (with `--report`) a machine-readable JSON dump.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SkipEntry {
    pub definition: String,
    pub member: String,
    pub reason: String,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct OverloadEntry {
    pub definition: String,
    pub member: String,
    pub chosen_arity: usize,
    pub dropped: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct UnknownTypeEntry {
    pub location: String,
    pub type_: String,
}

#[derive(Debug, Serialize)]
pub struct UnionEntry {
    pub name: String,
    pub members: Vec<String>,
    pub occurrences: usize,
}

#[derive(Debug, Serialize)]
pub struct RenameEntry {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Serialize)]
pub struct FlattenEntry {
    pub base: String,
    pub derived: String,
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub skipped: Vec<SkipEntry>,
    pub overloads: Vec<OverloadEntry>,
    pub unknown_types: Vec<UnknownTypeEntry>,
    /// Dictionary field / typedef target substituted with `object` because
    /// the real reference would make the WIT type graph recursive (WIT
    /// records can't be, even indirectly through `list<T>`), unlike WebIDL
    /// dictionaries.
    pub cyclic_types: Vec<UnknownTypeEntry>,
    pub unions: Vec<UnionEntry>,
    pub renames: Vec<RenameEntry>,
    pub flattened: Vec<FlattenEntry>,
    pub async_funcs: usize,
    pub futures: usize,
    pub integer_attrs: usize,
}

/// The text used both for the in-file `///` comment and the report's
/// `detail` field, so the two never drift apart.
pub fn omission_note(reason: &str, detail: &str) -> String {
    if detail.is_empty() {
        format!("omitted: {reason}")
    } else {
        format!("omitted: {reason} ({detail})")
    }
}

impl Report {
    pub fn skip(&mut self, definition: &str, member: &str, reason: &str, detail: &str) -> String {
        self.skipped.push(SkipEntry {
            definition: definition.to_string(),
            member: member.to_string(),
            reason: reason.to_string(),
            detail: detail.to_string(),
        });
        omission_note(reason, detail)
    }

    pub fn overload(&mut self, definition: &str, member: &str, chosen_arity: usize, dropped: Vec<String>) {
        if !dropped.is_empty() {
            self.overloads.push(OverloadEntry {
                definition: definition.to_string(),
                member: member.to_string(),
                chosen_arity,
                dropped,
            });
        }
    }

    pub fn unknown_type(&mut self, location: &str, type_: &str) {
        self.unknown_types.push(UnknownTypeEntry {
            location: location.to_string(),
            type_: type_.to_string(),
        });
    }

    pub fn cyclic_type(&mut self, location: &str, type_: &str) {
        self.cyclic_types.push(UnknownTypeEntry {
            location: location.to_string(),
            type_: type_.to_string(),
        });
    }

    pub fn renamed(&mut self, from: &str, to: &str) {
        self.renames.push(RenameEntry {
            from: from.to_string(),
            to: to.to_string(),
        });
    }

    pub fn flattened(&mut self, base: &str, derived: &str) {
        self.flattened.push(FlattenEntry {
            base: base.to_string(),
            derived: derived.to_string(),
        });
    }

    pub fn note_async(&mut self) {
        self.async_funcs += 1;
    }

    pub fn note_future(&mut self) {
        self.futures += 1;
    }

    pub fn note_integer_attr(&mut self) {
        self.integer_attrs += 1;
    }

    pub fn record_union(&mut self, name: &str, members: &[String]) {
        if let Some(existing) = self.unions.iter_mut().find(|u| u.name == name) {
            existing.occurrences += 1;
        } else {
            self.unions.push(UnionEntry {
                name: name.to_string(),
                members: members.to_vec(),
                occurrences: 1,
            });
        }
    }

    pub fn print_summary(&self) {
        let mut by_reason: Vec<(&str, usize)> = Vec::new();
        for s in &self.skipped {
            match by_reason.iter_mut().find(|(r, _)| *r == s.reason) {
                Some((_, n)) => *n += 1,
                None => by_reason.push((&s.reason, 1)),
            }
        }
        let breakdown = by_reason
            .iter()
            .map(|(r, n)| format!("{r}: {n}"))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("canonwit: {} member(s) skipped ({breakdown})", self.skipped.len());
        let dropped_overloads: usize = self.overloads.iter().map(|o| o.dropped.len()).sum();
        eprintln!(
            "           {} overload set(s) coalesced, {} overload(s) dropped",
            self.overloads.len(),
            dropped_overloads
        );
        eprintln!("           {} unknown type reference(s) -> object", self.unknown_types.len());
        eprintln!(
            "           {} recursive type reference(s) -> object (WIT records can't be recursive)",
            self.cyclic_types.len()
        );
        let union_occurrences: usize = self.unions.iter().map(|u| u.occurrences).sum();
        eprintln!(
            "           {} union(s) synthesized (deduped from {} occurrence(s))",
            self.unions.len(),
            union_occurrences
        );
        eprintln!("           {} interface/dictionary base(s) flattened", self.flattened.len());
        eprintln!("           {} name collision(s) renamed", self.renames.len());
        eprintln!("           {} async func(s), {} future<T> position(s)", self.async_funcs, self.futures);
        eprintln!("           {} [Clamp]/[EnforceRange] integer attribute(s) recognized (type unchanged)", self.integer_attrs);
    }
}
