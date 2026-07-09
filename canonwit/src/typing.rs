//! Maps a WebIDL type string (as stored by `indexer`, see `render.rs` in
//! that crate for the exact spellings) to a `wit_encoder::Type`. Strings
//! are re-parsed with `weedle` -- they're canonical WebIDL syntax already,
//! so no new grammar is needed. The match arms below mirror
//! `indexer/src/render.rs::non_any_type` variant-for-variant.

use anyhow::{ensure, Result};
use weedle::attribute::{ExtendedAttribute, ExtendedAttributeList};
use weedle::types::{
    AttributedType, FloatingPointType, IntegerType, NonAnyType, RecordKeyType, ReturnType,
    SingleType, Type as WType, UnionMemberType, UnionType,
};
use weedle::Parse;
use wit_encoder::Type as WitType;

use crate::names::NameRegistry;
use crate::report::Report;
use crate::symbols::{SymbolKind, SymbolTable, ANY, OBJECT, SYMBOL};
use crate::unions::UnionRegistry;

pub struct TypeCtx<'a> {
    pub symbols: &'a SymbolTable,
    pub names: &'a mut NameRegistry,
    pub unions: &'a mut UnionRegistry,
    pub report: &'a mut Report,
    /// Human-readable location for diagnostics, e.g. `"Element.className"`.
    pub location: String,
}

/// The result of mapping an operation's return-type string: whether the
/// function must become `async` (top-level `Promise<T>` return), and the
/// mapped result type (`None` for `undefined`/`Promise<undefined>`).
pub struct ReturnMapping {
    pub is_async: bool,
    pub result: Option<WitType>,
}

pub fn map_return_type_string(s: &str, ctx: &mut TypeCtx) -> Result<ReturnMapping> {
    let s = s.trim();
    // Some vendored WebIDL still uses the pre-standardization `void` return
    // keyword. weedle's grammar only knows `undefined`, so `void` would
    // otherwise silently parse as a dangling identifier type reference.
    if s == "void" {
        return Ok(ReturnMapping { is_async: false, result: None });
    }
    let (rest, rt) =
        ReturnType::parse(s).map_err(|e| anyhow::anyhow!("unparsable return type {s:?}: {e:?}"))?;
    ensure!(rest.trim().is_empty(), "trailing junk after return type {s:?}: {rest:?}");
    match rt {
        ReturnType::Undefined(_) => Ok(ReturnMapping { is_async: false, result: None }),
        ReturnType::Type(WType::Single(SingleType::NonAny(NonAnyType::Promise(p)))) => {
            ctx.report.note_async();
            Ok(ReturnMapping {
                is_async: true,
                result: map_return_type(&p.generics.body, ctx),
            })
        }
        ReturnType::Type(t) => Ok(ReturnMapping { is_async: false, result: Some(map_type(&t, ctx)) }),
    }
}

/// Maps an argument/dictionary-field/typedef type string, which may carry a
/// leading `[Clamp]`/`[EnforceRange]`/etc. extended-attribute list.
pub fn map_attributed_type_string(s: &str, ctx: &mut TypeCtx) -> Result<WitType> {
    let (rest, at) =
        AttributedType::parse(s.trim()).map_err(|e| anyhow::anyhow!("unparsable type {s:?}: {e:?}"))?;
    ensure!(rest.trim().is_empty(), "trailing junk after type {s:?}: {rest:?}");
    if has_flag_attr(&at.attributes, "Clamp") || has_flag_attr(&at.attributes, "EnforceRange") {
        ctx.report.note_integer_attr();
    }
    Ok(map_type(&at.type_, ctx))
}

fn has_flag_attr(attrs: &Option<ExtendedAttributeList<'_>>, flag: &str) -> bool {
    let Some(list) = attrs else { return false };
    list.body
        .list
        .iter()
        .any(|a| matches!(a, ExtendedAttribute::NoArgs(n) if n.0.0 == flag))
}

fn map_return_type(rt: &ReturnType<'_>, ctx: &mut TypeCtx) -> Option<WitType> {
    match rt {
        ReturnType::Undefined(_) => None,
        ReturnType::Type(t) => Some(map_type(t, ctx)),
    }
}

fn wrap_null(base: WitType, nullable: bool) -> WitType {
    if nullable {
        WitType::option(base)
    } else {
        base
    }
}

fn map_type(t: &WType<'_>, ctx: &mut TypeCtx) -> WitType {
    match t {
        WType::Single(SingleType::Any(_)) => WitType::named(ANY),
        WType::Single(SingleType::NonAny(nat)) => map_non_any_type(nat, ctx),
        WType::Union(m) => {
            let base = map_union_type(&m.type_, ctx);
            wrap_null(base, m.q_mark.is_some())
        }
    }
}

/// Buffer-source-family types with no clean WIT equivalent: represent as
/// bytes/elements. `list<u8>` for the untyped/byte-oriented ones.
fn map_non_any_type(t: &NonAnyType<'_>, ctx: &mut TypeCtx) -> WitType {
    macro_rules! scalar {
        ($m:expr, $wit:expr) => {
            wrap_null($wit, $m.q_mark.is_some())
        };
    }
    match t {
        NonAnyType::Promise(p) => {
            ctx.report.note_future();
            WitType::future(map_return_type(&p.generics.body, ctx))
        }
        NonAnyType::Integer(m) => scalar!(m, map_integer_type(&m.type_)),
        NonAnyType::FloatingPoint(m) => scalar!(m, map_float_type(&m.type_)),
        NonAnyType::Boolean(m) => scalar!(m, WitType::Bool),
        NonAnyType::Byte(m) => scalar!(m, WitType::S8),
        NonAnyType::Octet(m) => scalar!(m, WitType::U8),
        NonAnyType::ByteString(m) => scalar!(m, WitType::String),
        NonAnyType::DOMString(m) => scalar!(m, WitType::String),
        NonAnyType::USVString(m) => scalar!(m, WitType::String),
        NonAnyType::Object(m) => scalar!(m, WitType::named(OBJECT)),
        NonAnyType::Symbol(m) => scalar!(m, WitType::named(SYMBOL)),
        NonAnyType::Error(m) => scalar!(m, WitType::named(OBJECT)),
        NonAnyType::ArrayBuffer(m) => scalar!(m, WitType::list(WitType::U8)),
        NonAnyType::DataView(m) => scalar!(m, WitType::list(WitType::U8)),
        NonAnyType::Int8Array(m) => scalar!(m, WitType::list(WitType::S8)),
        NonAnyType::Int16Array(m) => scalar!(m, WitType::list(WitType::S16)),
        NonAnyType::Int32Array(m) => scalar!(m, WitType::list(WitType::S32)),
        NonAnyType::Uint8Array(m) => scalar!(m, WitType::list(WitType::U8)),
        NonAnyType::Uint16Array(m) => scalar!(m, WitType::list(WitType::U16)),
        NonAnyType::Uint32Array(m) => scalar!(m, WitType::list(WitType::U32)),
        NonAnyType::Uint8ClampedArray(m) => scalar!(m, WitType::list(WitType::U8)),
        NonAnyType::Float32Array(m) => scalar!(m, WitType::list(WitType::F32)),
        NonAnyType::Float64Array(m) => scalar!(m, WitType::list(WitType::F64)),
        NonAnyType::ArrayBufferView(m) => scalar!(m, WitType::list(WitType::U8)),
        NonAnyType::BufferSource(m) => scalar!(m, WitType::list(WitType::U8)),
        NonAnyType::Sequence(m) => {
            let inner = map_type(&m.type_.generics.body, ctx);
            scalar!(m, WitType::list(inner))
        }
        NonAnyType::FrozenArrayType(m) => {
            let inner = map_type(&m.type_.generics.body, ctx);
            scalar!(m, WitType::list(inner))
        }
        NonAnyType::ObservableArrayType(m) => {
            let inner = map_type(&m.type_.generics.body, ctx);
            scalar!(m, WitType::list(inner))
        }
        NonAnyType::RecordType(m) => {
            let key = map_record_key_type(&m.type_.generics.body.0, ctx);
            let value = map_type(&m.type_.generics.body.2, ctx);
            scalar!(m, WitType::list(WitType::tuple([key, value])))
        }
        NonAnyType::Identifier(m) => scalar!(m, resolve_identifier(m.type_.0, ctx)),
    }
}

fn map_integer_type(t: &IntegerType) -> WitType {
    match t {
        IntegerType::LongLong(t) => {
            if t.unsigned.is_some() { WitType::U64 } else { WitType::S64 }
        }
        IntegerType::Long(t) => {
            if t.unsigned.is_some() { WitType::U32 } else { WitType::S32 }
        }
        IntegerType::Short(t) => {
            if t.unsigned.is_some() { WitType::U16 } else { WitType::S16 }
        }
    }
}

fn map_float_type(t: &FloatingPointType) -> WitType {
    match t {
        FloatingPointType::Float(_) => WitType::F32,
        FloatingPointType::Double(_) => WitType::F64,
    }
}

fn map_record_key_type(t: &RecordKeyType<'_>, ctx: &mut TypeCtx) -> WitType {
    match t {
        RecordKeyType::Byte(_) => WitType::String,
        RecordKeyType::DOM(_) => WitType::String,
        RecordKeyType::USV(_) => WitType::String,
        RecordKeyType::NonAny(t) => map_non_any_type(t, ctx),
    }
}

fn map_union_type(t: &UnionType<'_>, ctx: &mut TypeCtx) -> WitType {
    let members: Vec<WitType> = t.body.list.iter().map(|m| map_union_member_type(m, ctx)).collect();
    ctx.unions.intern(members, ctx.names, ctx.report)
}

fn map_union_member_type(t: &UnionMemberType<'_>, ctx: &mut TypeCtx) -> WitType {
    match t {
        UnionMemberType::Single(t) => map_non_any_type(&t.type_, ctx),
        UnionMemberType::Union(m) => wrap_null(map_union_type(&m.type_, ctx), m.q_mark.is_some()),
    }
}

fn resolve_identifier(name: &str, ctx: &mut TypeCtx) -> WitType {
    // `bigint` predates weedle 0.13's grammar, so it parses as a dangling
    // identifier reference rather than a recognized primitive. WIT has no
    // arbitrary-precision integer, so `s64` is the closest (lossy) match.
    if name == "bigint" {
        return WitType::S64;
    }
    match ctx.symbols.kind_of(name) {
        Some(SymbolKind::Interface | SymbolKind::Dictionary | SymbolKind::Enum | SymbolKind::Typedef) => {
            match ctx.symbols.wit_name(name) {
                Some(wit_name) => WitType::named(wit_name.to_string()),
                None => WitType::named(OBJECT),
            }
        }
        Some(SymbolKind::Callback | SymbolKind::CallbackInterface) => WitType::named(OBJECT),
        Some(SymbolKind::Namespace) | None => {
            ctx.report.unknown_type(&ctx.location, name);
            WitType::named(OBJECT)
        }
    }
}

/// Parameters take resources by `borrow<r>` rather than owned `r` (returns
/// and getter results stay owned -- see `map_return_type_string`/`map_type`
/// above, which this is deliberately *not* wired into). Only rewrites a
/// bare resource reference at the top level or directly under `option<_>`;
/// a resource nested inside a `list`/`record`/union stays owned, since
/// `borrow<_>` isn't legal in a stored/composite position.
pub fn as_param_type(ty: WitType, symbols: &SymbolTable) -> WitType {
    match ty {
        WitType::Named(id) if symbols.is_resource_wit_name(id.raw_name()) => {
            WitType::borrow(id.raw_name().to_string())
        }
        WitType::Option(inner) => match *inner {
            WitType::Named(id) if symbols.is_resource_wit_name(id.raw_name()) => {
                WitType::option(WitType::borrow(id.raw_name().to_string()))
            }
            other => WitType::option(other),
        },
        other => other,
    }
}
