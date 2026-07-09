//! Renders weedle AST fragments back into canonical WebIDL-ish strings.
//!
//! We don't keep weedle's typed AST around (see `model.rs`), so types,
//! extended attributes, and default values are flattened to strings here.
//! This loses nothing that downstream diffing cares about: two
//! syntactically different but semantically identical spellings are rare in
//! practice, and browsers vendor their IDL through similar tooling anyway.

use weedle::argument::{Argument as WArgument, ArgumentList};
use weedle::attribute::{ExtendedAttribute, ExtendedAttributeList, IdentifierOrString};
use weedle::common::Default as WDefault;
use weedle::literal::{ConstValue, DefaultValue, FloatLit, IntegerLit};
use weedle::types::{
    ConstType, FloatingPointType, IntegerType, NonAnyType, RecordKeyType, ReturnType, Type,
    UnionMemberType,
};

pub fn extended_attributes(list: &Option<ExtendedAttributeList<'_>>) -> Vec<String> {
    let Some(list) = list else {
        return Vec::new();
    };
    list.body.list.iter().map(extended_attribute).collect()
}

fn extended_attribute(attr: &ExtendedAttribute<'_>) -> String {
    match attr {
        ExtendedAttribute::NoArgs(a) => a.0.0.to_string(),
        ExtendedAttribute::Ident(a) => {
            format!("{}={}", a.lhs_identifier.0, ident_or_string(&a.rhs))
        }
        ExtendedAttribute::Wildcard(a) => format!("{}=*", a.lhs_identifier.0),
        ExtendedAttribute::IdentList(a) => format!(
            "{}=({})",
            a.identifier.0,
            a.list
                .body
                .list
                .iter()
                .map(|i| i.0.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ExtendedAttribute::ArgList(a) => format!(
            "{}({})",
            a.identifier.0,
            argument_list(&a.args.body).join(", ")
        ),
        ExtendedAttribute::NamedArgList(a) => format!(
            "{}={}({})",
            a.lhs_identifier.0,
            a.rhs_identifier.0,
            argument_list(&a.args.body).join(", ")
        ),
    }
}

fn ident_or_string(v: &IdentifierOrString<'_>) -> String {
    match v {
        IdentifierOrString::Identifier(id) => id.0.to_string(),
        IdentifierOrString::String(s) => format!("\"{}\"", s.0),
    }
}

fn ext_attr_prefix(list: &Option<ExtendedAttributeList<'_>>) -> String {
    let attrs = extended_attributes(list);
    if attrs.is_empty() {
        String::new()
    } else {
        format!("[{}] ", attrs.join(", "))
    }
}

pub fn argument_list(list: &ArgumentList<'_>) -> Vec<String> {
    list.list.iter().map(argument_string).collect()
}

fn argument_string(arg: &WArgument<'_>) -> String {
    match arg {
        WArgument::Single(a) => {
            let mut s = ext_attr_prefix(&a.attributes);
            if a.optional.is_some() {
                s.push_str("optional ");
            }
            s.push_str(&attributed_type(&a.type_));
            s.push(' ');
            s.push_str(a.identifier.0);
            if let Some(d) = &a.default {
                s.push_str(" = ");
                s.push_str(&default_value(&d.value));
            }
            s
        }
        WArgument::Variadic(a) => {
            let mut s = ext_attr_prefix(&a.attributes);
            s.push_str(&type_(&a.type_));
            s.push_str("... ");
            s.push_str(a.identifier.0);
            s
        }
    }
}

pub fn arguments(list: &ArgumentList<'_>) -> Vec<crate::model::Argument> {
    list.list
        .iter()
        .map(|arg| match arg {
            WArgument::Single(a) => crate::model::Argument {
                name: a.identifier.0.to_string(),
                type_: attributed_type(&a.type_),
                optional: a.optional.is_some(),
                variadic: false,
                default: a.default.as_ref().map(|d| default_value(&d.value)),
                extended_attributes: extended_attributes(&a.attributes),
            },
            WArgument::Variadic(a) => crate::model::Argument {
                name: a.identifier.0.to_string(),
                type_: type_(&a.type_),
                optional: false,
                variadic: true,
                default: None,
                extended_attributes: extended_attributes(&a.attributes),
            },
        })
        .collect()
}

pub fn attributed_type(t: &weedle::types::AttributedType<'_>) -> String {
    format!("{}{}", ext_attr_prefix(&t.attributes), type_(&t.type_))
}

fn attributed_non_any_type(t: &weedle::types::AttributedNonAnyType<'_>) -> String {
    format!("{}{}", ext_attr_prefix(&t.attributes), non_any_type(&t.type_))
}

pub fn return_type(t: &ReturnType<'_>) -> String {
    match t {
        ReturnType::Undefined(_) => "undefined".to_string(),
        ReturnType::Type(t) => type_(t),
    }
}

pub fn const_type(t: &ConstType<'_>) -> String {
    match t {
        ConstType::Integer(m) => suffix_null(&integer_type(&m.type_), m.q_mark.is_some()),
        ConstType::FloatingPoint(m) => {
            suffix_null(&floating_point_type(&m.type_), m.q_mark.is_some())
        }
        ConstType::Boolean(m) => suffix_null("boolean", m.q_mark.is_some()),
        ConstType::Byte(m) => suffix_null("byte", m.q_mark.is_some()),
        ConstType::Octet(m) => suffix_null("octet", m.q_mark.is_some()),
        ConstType::Identifier(m) => suffix_null(m.type_.0, m.q_mark.is_some()),
    }
}

pub fn const_value(v: &ConstValue<'_>) -> String {
    match v {
        ConstValue::Boolean(b) => b.0.to_string(),
        ConstValue::Float(f) => float_lit(f),
        ConstValue::Integer(i) => integer_lit(i).to_string(),
        ConstValue::Null(_) => "null".to_string(),
    }
}

fn default_value(v: &DefaultValue<'_>) -> String {
    match v {
        DefaultValue::Boolean(b) => b.0.to_string(),
        DefaultValue::EmptyArray(_) => "[]".to_string(),
        DefaultValue::EmptyDictionary(_) => "{}".to_string(),
        DefaultValue::Float(f) => float_lit(f),
        DefaultValue::Integer(i) => integer_lit(i).to_string(),
        DefaultValue::Null(_) => "null".to_string(),
        DefaultValue::String(s) => format!("\"{}\"", s.0),
    }
}

fn float_lit(f: &FloatLit<'_>) -> String {
    match f {
        FloatLit::Value(v) => v.0.to_string(),
        FloatLit::NegInfinity(_) => "-Infinity".to_string(),
        FloatLit::Infinity(_) => "Infinity".to_string(),
        FloatLit::NaN(_) => "NaN".to_string(),
    }
}

fn integer_lit(i: &IntegerLit<'_>) -> String {
    match i {
        IntegerLit::Dec(v) => v.0.to_string(),
        IntegerLit::Hex(v) => v.0.to_string(),
        IntegerLit::Oct(v) => v.0.to_string(),
    }
}

pub fn default_rhs(d: &Option<WDefault<'_>>) -> Option<String> {
    d.as_ref().map(|d| default_value(&d.value))
}

fn suffix_null(s: &str, nullable: bool) -> String {
    if nullable {
        format!("{s}?")
    } else {
        s.to_string()
    }
}

fn integer_type(t: &IntegerType) -> String {
    match t {
        IntegerType::LongLong(t) => {
            format!("{}long long", if t.unsigned.is_some() { "unsigned " } else { "" })
        }
        IntegerType::Long(t) => {
            format!("{}long", if t.unsigned.is_some() { "unsigned " } else { "" })
        }
        IntegerType::Short(t) => {
            format!("{}short", if t.unsigned.is_some() { "unsigned " } else { "" })
        }
    }
}

fn floating_point_type(t: &FloatingPointType) -> String {
    match t {
        FloatingPointType::Float(t) => format!(
            "{}float",
            if t.unrestricted.is_some() { "unrestricted " } else { "" }
        ),
        FloatingPointType::Double(t) => format!(
            "{}double",
            if t.unrestricted.is_some() { "unrestricted " } else { "" }
        ),
    }
}

fn record_key_type(t: &RecordKeyType<'_>) -> String {
    match t {
        RecordKeyType::Byte(_) => "ByteString".to_string(),
        RecordKeyType::DOM(_) => "DOMString".to_string(),
        RecordKeyType::USV(_) => "USVString".to_string(),
        RecordKeyType::NonAny(t) => non_any_type(t),
    }
}

fn non_any_type(t: &NonAnyType<'_>) -> String {
    match t {
        NonAnyType::Promise(p) => format!("Promise<{}>", return_type(&p.generics.body)),
        NonAnyType::Integer(m) => suffix_null(&integer_type(&m.type_), m.q_mark.is_some()),
        NonAnyType::FloatingPoint(m) => {
            suffix_null(&floating_point_type(&m.type_), m.q_mark.is_some())
        }
        NonAnyType::Boolean(m) => suffix_null("boolean", m.q_mark.is_some()),
        NonAnyType::Byte(m) => suffix_null("byte", m.q_mark.is_some()),
        NonAnyType::Octet(m) => suffix_null("octet", m.q_mark.is_some()),
        NonAnyType::ByteString(m) => suffix_null("ByteString", m.q_mark.is_some()),
        NonAnyType::DOMString(m) => suffix_null("DOMString", m.q_mark.is_some()),
        NonAnyType::USVString(m) => suffix_null("USVString", m.q_mark.is_some()),
        NonAnyType::Sequence(m) => suffix_null(
            &format!("sequence<{}>", type_(&m.type_.generics.body)),
            m.q_mark.is_some(),
        ),
        NonAnyType::Object(m) => suffix_null("object", m.q_mark.is_some()),
        NonAnyType::Symbol(m) => suffix_null("symbol", m.q_mark.is_some()),
        NonAnyType::Error(m) => suffix_null("Error", m.q_mark.is_some()),
        NonAnyType::ArrayBuffer(m) => suffix_null("ArrayBuffer", m.q_mark.is_some()),
        NonAnyType::DataView(m) => suffix_null("DataView", m.q_mark.is_some()),
        NonAnyType::Int8Array(m) => suffix_null("Int8Array", m.q_mark.is_some()),
        NonAnyType::Int16Array(m) => suffix_null("Int16Array", m.q_mark.is_some()),
        NonAnyType::Int32Array(m) => suffix_null("Int32Array", m.q_mark.is_some()),
        NonAnyType::Uint8Array(m) => suffix_null("Uint8Array", m.q_mark.is_some()),
        NonAnyType::Uint16Array(m) => suffix_null("Uint16Array", m.q_mark.is_some()),
        NonAnyType::Uint32Array(m) => suffix_null("Uint32Array", m.q_mark.is_some()),
        NonAnyType::Uint8ClampedArray(m) => suffix_null("Uint8ClampedArray", m.q_mark.is_some()),
        NonAnyType::Float32Array(m) => suffix_null("Float32Array", m.q_mark.is_some()),
        NonAnyType::Float64Array(m) => suffix_null("Float64Array", m.q_mark.is_some()),
        NonAnyType::ArrayBufferView(m) => suffix_null("ArrayBufferView", m.q_mark.is_some()),
        NonAnyType::BufferSource(m) => suffix_null("BufferSource", m.q_mark.is_some()),
        NonAnyType::FrozenArrayType(m) => suffix_null(
            &format!("FrozenArray<{}>", type_(&m.type_.generics.body)),
            m.q_mark.is_some(),
        ),
        NonAnyType::ObservableArrayType(m) => suffix_null(
            &format!("ObservableArray<{}>", type_(&m.type_.generics.body)),
            m.q_mark.is_some(),
        ),
        NonAnyType::RecordType(m) => suffix_null(
            &format!(
                "record<{}, {}>",
                record_key_type(&m.type_.generics.body.0),
                type_(&m.type_.generics.body.2)
            ),
            m.q_mark.is_some(),
        ),
        NonAnyType::Identifier(m) => suffix_null(m.type_.0, m.q_mark.is_some()),
    }
}

fn union_member_type(t: &UnionMemberType<'_>) -> String {
    match t {
        UnionMemberType::Single(t) => attributed_non_any_type(t),
        UnionMemberType::Union(m) => suffix_null(&union_type(&m.type_), m.q_mark.is_some()),
    }
}

fn union_type(t: &weedle::types::UnionType<'_>) -> String {
    format!(
        "({})",
        t.body
            .list
            .iter()
            .map(union_member_type)
            .collect::<Vec<_>>()
            .join(" or ")
    )
}

pub fn type_(t: &Type<'_>) -> String {
    match t {
        Type::Single(weedle::types::SingleType::Any(_)) => "any".to_string(),
        Type::Single(weedle::types::SingleType::NonAny(t)) => non_any_type(t),
        Type::Union(m) => suffix_null(&union_type(&m.type_), m.q_mark.is_some()),
    }
}
