//! Packet parsing code generation.

use std::{borrow::Cow, fmt::Write};

use crate::parse::{Condition, Field, Kind, Packet};

pub fn emit(w: &mut impl Write, packets: &[Packet]) -> anyhow::Result<()> {
    w.write_str("//! LoA packet structures.\n\n")?;
    super::emit_notice(w)?;
    writeln!(
        w,
        "use crate::parser::{{BumpVec, Event, Packet, Parser, KindedBytes, serialize_bumpvec}};"
    )?;
    writeln!(w, "use super::opcode::Opcode;")?;
    writeln!(
        w,
        "use crate::definitions::{{TripodIndex, TripodLevel, SkillOptionData, SkillMoveOptionData}};"
    )?;
    writeln!(w)?;
    for packet in packets {
        // println!("{:#?}", packet);
        emit_struct(w, packets, &packet)?;
    }
    Ok(())
}

/// Given fields, filter for only captured fields (i.e. named fields).
fn captured(fields: &[Field]) -> impl Iterator<Item = (&str, &Kind)> + '_ {
    fields
        .iter()
        .filter_map(|f| f.name.as_deref().map(|n| (n, &f.kind)))
}

fn has_captured_fields(fields: &[Field]) -> bool {
    captured(fields).next().is_some()
}

// NOTE: Fields contain references to other packets (`Kind::Struct`), which mandates
//       carrying around the list of packets to determine lifetime requirements.

// Lifetime annotations are required on structs recursively containing a `Kind::Array`
// or a `Kind::String` -- both depending on a bump allocation for dynamic memory.

fn any_fields_need_lifetime(packets: &[Packet], fields: &[Field]) -> bool {
    captured(&fields).any(|(_, k)| kind_needs_lifetime(packets, k))
}

fn packet_needs_lifetime(packets: &[Packet], name: &str) -> bool {
    packets
        .iter()
        .find(|p| &p.name == name)
        .map(|p| any_fields_need_lifetime(packets, &p.fields))
        .unwrap_or(false)
}

fn kind_needs_lifetime(packets: &[Packet], kind: &Kind) -> bool {
    match kind {
        Kind::String(_) => true,
        Kind::Optional(_, kind) => kind_needs_lifetime(packets, kind),
        Kind::Struct(name) => packet_needs_lifetime(packets, name),
        Kind::Tuple(fs) => any_fields_need_lifetime(packets, fs),
        Kind::Array { .. } => true,
        _ => false,
    }
}

fn emit_struct(w: &mut impl Write, packets: &[Packet], packet: &Packet) -> anyhow::Result<()> {
    write!(w, "#[derive(serde::Serialize)]")?;
    write!(w, "pub struct {}", packet.name)?;
    let has_lifetime = any_fields_need_lifetime(packets, &packet.fields);
    if has_lifetime {
        w.write_str("<'bump>")?;
    }
    if has_captured_fields(&packet.fields) {
        w.write_str(" {\n")?;
        for (name, kind) in captured(&packet.fields) {
            if matches!(kind, Kind::Array { .. }) {
                writeln!(w, "    #[serde(serialize_with = \"serialize_bumpvec\")]")?;
            }
            writeln!(w, "    pub {}: {},", name, kind.rust_type(packets))?;
        }
        w.write_str("}\n")?;
    } else {
        w.write_str(";\n")?;
    }
    w.write_char('\n')?;

    if packet.opcode.is_some() {
        writeln!(w, "\nimpl Packet for {}", packet.name)?;
        if has_lifetime {
            w.write_str("<'_>")?;
        }
        w.write_str(" {\n")?;
        writeln!(
            w,
            "    const OPCODE: Opcode = Opcode::{};",
            &packet.name[3..]
        )?;
        w.write_str("}\n\n")?;
    }

    write!(w, "impl<'bump> Event<'bump> for {}", packet.name)?;
    if has_lifetime {
        w.write_str("<'bump>")?;
    }
    w.write_str(" {\n")?;
    w.write_str("    fn parse(parser: &mut Parser, ")?;
    w.write_str(if uses_bump(packet) { "bump" } else { "_" })?;
    w.write_str(": &'bump bumpalo::Bump) -> anyhow::Result<Self> {\n")?;

    emit_fields(w, packets, &packet.fields)?;

    if has_captured_fields(&packet.fields) {
        w.write_str("        Ok(Self {\n")?;
        for (name, _) in captured(&packet.fields) {
            writeln!(w, "            {},", name)?;
        }
        w.write_str("        })\n")?;
    } else {
        w.write_str("        Ok(Self)\n")?;
    }
    w.write_str("    }\n")?;
    w.write_str("}\n")?;
    w.write_char('\n')?;

    Ok(())
}

fn emit_fields(w: &mut impl Write, packets: &[Packet], fields: &[Field]) -> anyhow::Result<()> {
    for field in fields {
        emit_field(w, packets, field)?;
    }
    Ok(())
}

fn emit_field(w: &mut impl Write, packets: &[Packet], field: &Field) -> anyhow::Result<()> {
    if let Some(name) = &field.name {
        write!(w, "let {} = ", name)?;
    }
    emit_kind(w, packets, &field.kind)?;
    w.write_str(";\n")?;
    Ok(())
}

fn uses_bump(packet: &Packet) -> bool {
    packet.fields.iter().any(|f| {
        matches!(
            f.kind,
            Kind::String(_)
                | Kind::Optional(..)
                | Kind::Struct(_)
                | Kind::Bytes(_)
                | Kind::KindedBytes(..)
                | Kind::Array { .. }
        )
    })
}

fn emit_kind(w: &mut impl Write, packets: &[Packet], kind: &Kind) -> anyhow::Result<()> {
    match kind {
        Kind::U8 => w.write_str("parser.read_u8()?")?,
        Kind::U16 => w.write_str("parser.read_u16()?")?,
        Kind::U32 => w.write_str("parser.read_u32()?")?,
        Kind::U64 => w.write_str("parser.read_u64()?")?,
        Kind::I8 => w.write_str("parser.read_i8()?")?,
        Kind::I16 => w.write_str("parser.read_i16()?")?,
        Kind::I32 => w.write_str("parser.read_i32()?")?,
        Kind::I64 => w.write_str("parser.read_i64()?")?,
        Kind::F32 => w.write_str("parser.read_f32()?")?,
        Kind::Bool => w.write_str("parser.read_bool()?")?,
        Kind::String(_) => w.write_str("parser.read_str(bump)?")?,
        Kind::PackedI64 => w.write_str("parser.read_packed_i64()?")?,
        Kind::DateTime => w.write_str("parser.read_simple_u64()?")?,
        Kind::Angle => w.write_str("parser.read_u16()?")?,
        Kind::Vector => w.write_str("parser.read_u64()?")?,
        Kind::Optional(cond, kind) => {
            match cond {
                Condition::Bool => write!(
                    w,
                    "(parser.read_bool()?).then(|| <{}>::parse(parser, bump)).transpose()?",
                    kind.rust_type_nl()
                )?,
                Condition::Equality(name, lit) => write!(
                    w,
                    "({} == {}).then(|| <{}>::parse(parser, bump)).transpose()?",
                    name,
                    lit,
                    kind.rust_type_nl()
                )?,
                Condition::Greater(name, lit) => write!(
                    w,
                    "({} > {}).then(|| <{}>::parse(parser, bump)).transpose()?",
                    name,
                    lit,
                    kind.rust_type_nl()
                )?,
            }
            // write!(w, "parser.read_optional::<{}>(bump)?", kind.rust_type_nl())?
        }
        Kind::If(..) => unreachable!(),
        Kind::Struct(name) => write!(w, "<{}>::parse(parser, bump)?", name)?,
        Kind::Bytes(len) => write!(w, "<[u8; {}]>::parse(parser, bump)?", len)?,
        Kind::KindedBytes(len_kind, max_len, mult) => {
            write!(
                w,
                "KindedBytes::<{}, {}, {}>::parse(parser, bump)?",
                len_kind.rust_type_nl(),
                mult.unwrap_or(1),
                max_len
            )?;
        }
        Kind::Array {
            kind,
            len_kind,
            len,
        } => {
            write!(
                w,
                "parser.read_counted::<{}, {}>(bump, {})?",
                kind.rust_type_nl(),
                len_kind.rust_type_nl(),
                len,
            )?;
        }
        Kind::Skip(count) => write!(w, "parser.skip({})?", count)?,
        Kind::Tuple(_) => unreachable!(),
    }
    Ok(())
}

impl Kind {
    // no lifetime
    fn rust_type_nl(&self) -> Cow<str> {
        match self {
            Kind::String(_) => "&str".into(),
            Kind::Optional(_, kind) => format!("Option<{}>", kind.rust_type_nl()).into(),
            Kind::Struct(name) => name.into(),
            Kind::Array { kind, .. } => format!("BumpVec<{}>", kind.rust_type_nl()).into(),
            Kind::Tuple(_) => unreachable!(),
            Kind::KindedBytes(len_kind, max_len, mult) => format!(
                "KindedBytes<{}, {}, {}>",
                len_kind.rust_type_nl(),
                mult.unwrap_or(1),
                max_len
            )
            .into(),
            _ => self.rust_type(&[]),
        }
    }

    fn rust_type(&self, packets: &[Packet]) -> Cow<str> {
        match self {
            Kind::U8 => "u8".into(),
            Kind::U16 => "u16".into(),
            Kind::U32 => "u32".into(),
            Kind::U64 => "u64".into(),
            Kind::I8 => "i8".into(),
            Kind::I16 => "i16".into(),
            Kind::I32 => "i32".into(),
            Kind::I64 => "i64".into(),
            Kind::F32 => "f32".into(),
            Kind::Bool => "bool".into(),
            Kind::String(_) => "&'bump str".into(),
            Kind::PackedI64 => "i64".into(),
            Kind::DateTime => "u64".into(),
            Kind::Angle => "u16".into(),
            Kind::Vector => "u64".into(),
            Kind::Optional(_, kind) => format!("Option<{}>", kind.rust_type(packets)).into(),
            Kind::Struct(name) => {
                if kind_needs_lifetime(packets, self) {
                    let mut rty = name.clone();
                    rty.push_str("<'bump>");
                    rty.into()
                } else {
                    name.into()
                }
            }
            Kind::Bytes(len) => format!("[u8; {}]", len).into(),
            Kind::KindedBytes(..) => "()".into(),
            Kind::Array { kind, .. } => {
                format!("BumpVec<'bump, {}>", kind.rust_type(packets)).into()
            }
            Kind::Skip(_) => "()".into(),
            Kind::If(..) => unreachable!(),
            Kind::Tuple(_) => unreachable!(),
        }
    }
}
