//! Parser to extract the structure of packets from another project's packet parsing routines.

use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

use chumsky::prelude::*;
use heck::{ToPascalCase, ToSnekCase};

/// Packet or subpacket.
#[derive(Debug)]
pub struct Packet {
    pub name: String,
    pub fields: Vec<Field>,
    /// Subpackets do not have an opcode.
    pub opcode: Option<u16>,
}

/// Extract structural data of packets from given files.
pub fn parse_packets<'a>(file: impl Iterator<Item = impl AsRef<std::path::Path>>) -> Vec<Packet> {
    let packets: Vec<Packet> = file
        .map(|p| fs::read_to_string(p.as_ref()).expect("failed to read path"))
        .map(|v| parse_packet(&v))
        .collect();
    postprocess(packets)
}

fn parse_packet(src: &str) -> Packet {
    println!("{}", src);
    let packet = match parser().parse(&*src) {
        Ok(out) => out,
        Err(errs) => {
            for e in errs.into_iter() {
                let span = e.span();
                let line = src[..span.end].chars().filter(|&x| x == '\n').count();
                println!("{:?} - line {}", e, line);
            }
            std::process::exit(1);
        }
    };

    packet
}

fn postprocess(packets: Vec<Packet>) -> Vec<Packet> {
    let mut packets = lift_tuples_and_convert_builtins(packets);

    for packet in packets.iter_mut() {
        let mut required = vec![];
        find_used_idents(&packet.fields, &mut required);
        strip_generated_names(&mut packet.fields, &required);

        // debug printouts
        println!("{:?}", packet.name);
        if let Some(opcode) = packet.opcode {
            println!("{:?}", opcode);
        }
        for field in &packet.fields {
            println!("{:?}", field);
        }
        println!();
    }
    packets
}

/// Lift [`Kind::Tuple`]s (anonymous structures) into (sub-)[`Packet`]s (named structures).
/// Also converts named builtin (non-subpacket) structs into [`Kind`] equivalents.
/// These operations are grouped because they both involve recursing through all nested `Kind`s in
/// a packet's fields.
fn lift_tuples_and_convert_builtins(packets: Vec<Packet>) -> Vec<Packet> {
    static SUB_PACKET_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn recurse_kinds(kind: &mut Kind, out_packets: &mut Vec<Packet>) {
        match kind {
            Kind::Tuple(fields) => {
                for field in fields.iter_mut() {
                    recurse_kinds(&mut field.kind, out_packets);
                }

                let name = format!("Sub{}", SUB_PACKET_COUNTER.fetch_add(1, Ordering::Relaxed));
                let new_packet = Packet {
                    name: name.clone(),
                    fields: fields.clone(),
                    opcode: None,
                };
                out_packets.push(new_packet);

                *kind = Kind::Struct(name);
            }
            Kind::If(cond, fields) => {
                for field in fields.iter_mut() {
                    recurse_kinds(&mut field.kind, out_packets);
                }

                let name = format!("Sub{}", SUB_PACKET_COUNTER.fetch_add(1, Ordering::Relaxed));
                let new_packet = Packet {
                    name: name.clone(),
                    fields: fields.clone(),
                    opcode: None,
                };
                out_packets.push(new_packet);

                *kind = Kind::Optional(cond.clone(), Box::new(Kind::Struct(name)));
            }
            Kind::Struct(s) => match s.as_str() {
                "ReadNBytesInt64" => *kind = Kind::PackedI64,
                "Angle" => *kind = Kind::Angle,
                "LostArkDateTime" => *kind = Kind::DateTime,
                "Vector3F" => *kind = Kind::Vector,
                _ => {}
            },
            Kind::Optional(_, kind) => recurse_kinds(kind, out_packets),
            Kind::KindedBytes(kind, _, _) => recurse_kinds(kind, out_packets),
            Kind::Array { kind, .. } => recurse_kinds(kind, out_packets),
            _ => {}
        }
    }

    let mut out_packets = Vec::new();
    for mut packet in packets {
        for field in &mut packet.fields {
            recurse_kinds(&mut field.kind, &mut out_packets);
        }
        out_packets.push(packet);
    }

    out_packets
}

fn find_used_idents(fields: &[Field], out: &mut Vec<String>) {
    fn recurse_fields(kind: &Kind, out: &mut Vec<String>) {
        match kind {
            Kind::Optional(_, kind) => recurse_fields(kind, out),
            Kind::KindedBytes(kind, _, _) => recurse_fields(kind, out),
            Kind::Tuple(fields) => find_used_idents(fields, out),
            Kind::Array { kind, .. } => recurse_fields(kind, out),
            Kind::If(_, fields) => find_used_idents(fields, out),
            _ => {}
        }
    }

    for field in fields {
        match &field.kind {
            Kind::If(cond, _) | Kind::Optional(cond, _) => match cond {
                Condition::Equality(name, _) | Condition::Greater(name, _) => {
                    out.push(name.clone());
                }
                _ => {}
            },
            Kind::Array {
                len: LiteralOrIdent::Ident(name),
                ..
            } => {
                out.push(name.clone());
            }
            _ => {}
        }

        recurse_fields(&field.kind, out);
    }
}

/// Remove any automatically generated names, i.e. `unk0`.
fn strip_generated_names(fields: &mut [Field], required: &[String]) {
    fn recurse_fields(kind: &mut Kind, required: &[String]) {
        match kind {
            Kind::Optional(_, kind) => recurse_fields(kind, required),
            Kind::KindedBytes(kind, _, _) => recurse_fields(kind, required),
            Kind::Tuple(fields) => strip_generated_names(fields, required),
            Kind::Array { kind, .. } => recurse_fields(kind, required),
            Kind::If(_, fields) => strip_generated_names(fields, required),
            _ => {}
        }
    }

    const GENERATED: [&str; 5] = [
        "unk",
        "struct",
        "read_n",
        "lost_ark_string",
        "lost_ark_date_time",
    ];

    if !required.is_empty() {
        println!("{:#?}", required);
    }
    for field in fields.iter_mut() {
        recurse_fields(&mut field.kind, required);

        if let Some(name) = &field.name {
            // println!("THE NAME IS {}", name);
            if required.contains(&name) {
                // panic!("{}", name);
                continue;
            }

            if GENERATED.into_iter().any(|p| name.starts_with(p)) {
                field.name = None;
            }

            if field.name.as_deref() == Some("type") {
                field.name = Some("r#type".to_string());
            }
        }
    }
}

/// A field in a packet structure: its kind and an optional name.
#[derive(Debug, Clone)]
pub struct Field {
    pub kind: Kind,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Condition {
    Bool,
    Equality(String, usize),
    Greater(String, usize),
}

#[derive(Debug, Clone)]
pub enum LiteralOrIdent {
    Literal(u64),
    Ident(String),
}

impl std::fmt::Display for LiteralOrIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LiteralOrIdent::Literal(l) => write!(f, "{l}"),
            LiteralOrIdent::Ident(s) => f.write_str(&s),
        }
    }
}

/// Type of packet field.
#[derive(Debug, Clone)]
pub enum Kind {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    Bool,
    String(usize),
    PackedI64,
    DateTime,
    Angle,
    Vector,
    Optional(Condition, Box<Kind>),
    If(Condition, Vec<Field>),
    Struct(String),
    Bytes(usize),
    KindedBytes(Box<Kind>, usize, Option<usize>),
    Tuple(Vec<Field>),
    Array {
        kind: Box<Kind>,
        len_kind: Box<Kind>,
        len: LiteralOrIdent,
    },
    Skip(usize),
}

impl Kind {
    fn from_primitive_name(s: &str) -> Option<Self> {
        dbg!(s);
        Some(match s {
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "i8" => Self::I8,
            "i16" => Self::I16,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "f32" => Self::F32,
            "bool" => Self::Bool,
            _ => return None,
        })
    }
}

/// A `chumsky` parser for `lost-ark-dev/meter-core`'s parsing format.
// Won't hold up to changes in their format, but should have enough
// constructs to be able to adapt to minor changes fairly easily.
fn parser() -> impl Parser<char, Packet, Error = Simple<char>> + Clone {
    let semi = just(';');
    let comma = just(',');
    let arrow = just("=>");
    let open_block = just('{');
    let close_block = just('}');
    let block = recursive::<_, _, _, _, Simple<char>>(|block| {
        block
            .padded_by(none_of("{}").repeated())
            .repeated()
            .padded_by(none_of("{}").repeated())
            .delimited_by(open_block, close_block)
            .then(semi.or_not())
            .ignored()
    });
    let string = text::ident::<_, Simple<char>>().padded_by(just('"'));
    let ident = text::ident::<_, Simple<char>>().or(just('$').map(|c| c.to_string()));

    let comment = just("//").then(take_until(just('\n'))).padded();
    let block_comment = just("/*").then(take_until(just("*/"))).padded();
    let import = just("import").then(take_until(semi)).padded();
    let export = just("export type")
        .padded()
        .ignore_then(ident)
        .then_ignore(take_until(block.clone()))
        .map(|name| name.to_pascal_case());

    // Yields a `Vec<Field>`.
    let parse_block = recursive::<_, _, _, _, Simple<char>>(|pblock| {
        // Yields a `Kind`.
        let reader_call = recursive::<_, _, _, _, Simple<char>>(|rc| {
            let array = just("array(")
                .padded()
                .ignore_then(rc.clone())
                .then_ignore(comma.padded())
                .then_ignore(take_until(arrow).padded())
                .then(
                    pblock
                        .clone()
                        .map(|fields| Kind::Tuple(fields))
                        .or(rc.clone()),
                )
                .then_ignore(comma.padded())
                .then(text::int::<_, Simple<char>>(10).map(|s| s.parse::<usize>().unwrap()))
                .then_ignore(just(')').padded())
                .map(|((len_kind, field_kind), len)| Kind::Array {
                    kind: Box::new(field_kind),
                    len_kind: Box::new(len_kind),
                    len: LiteralOrIdent::Literal(len as u64),
                });

            let string = just("string(")
                .ignore_then(text::int::<_, Simple<char>>(10).map(|s| s.parse::<usize>().unwrap()))
                .map(Kind::String)
                .then_ignore(just(')'));

            // Possibilities
            // bytes(12) - just length
            // bytes(reader.u16(), 5) - length kind to read and max length
            // bytes(reader.u16(), 5, 7) - length kind to read and multiplier

            let bytes = just("bytes(")
                .ignore_then(
                    text::int::<_, Simple<char>>(10)
                        .map(|s| s.parse::<usize>().unwrap())
                        .map(Kind::Bytes)
                        .or(ident.map(|ident| Kind::Array {
                            kind: Box::new(Kind::U8),
                            len_kind: Box::new(Kind::I64),
                            len: LiteralOrIdent::Ident(ident),
                        })),
                )
                .then_ignore(just(')'));

            let kinded_bytes = just("bytes(")
                .ignore_then(rc.then_ignore(comma.padded()))
                .then(text::int::<_, Simple<char>>(10).map(|s| s.parse::<usize>().unwrap()))
                .then(
                    comma
                        .padded()
                        .ignore_then(
                            text::int::<_, Simple<char>>(10).map(|s| s.parse::<usize>().unwrap()),
                        )
                        .or_not(),
                )
                .map(|((k, len), other_len)| Kind::KindedBytes(Box::new(k), len, other_len))
                .then_ignore(just(')'));

            let skip = just("skip(")
                .ignore_then(text::int::<_, Simple<char>>(10).map(|s| s.parse::<usize>().unwrap()))
                .map(Kind::Skip)
                .then_ignore(just(')'));

            choice((
                just("reader.").ignore_then(choice((
                    array,
                    bytes,
                    kinded_bytes,
                    string,
                    skip,
                    // TODO: bytes and string
                    text::ident::<char, _>()
                        .map(|s| Kind::from_primitive_name(&s).unwrap())
                        .then_ignore(just("()")),
                ))),
                text::ident()
                    .map(|s: String| Kind::Struct(s.to_pascal_case()))
                    .then_ignore(just(".read(reader)")),
            ))
        });

        let assignment = just("const")
            .padded()
            .ignore_then(ident.padded())
            .or(ident
                .padded()
                .ignore_then(just("."))
                .ignore_then(text::ident()))
            .then_ignore(just('=').padded())
            .map(|s| s.to_snek_case());

        let if_stmt = just("if")
            .padded()
            .ignore_then(
                just('(')
                    .padded()
                    .ignore_then(
                        just("reader.bool()")
                            .map(|_| Condition::Bool)
                            .then_ignore(block_comment.or_not())
                            .or(ident
                                .then(just("===").or(just(">")).padded())
                                .then(
                                    text::int::<_, Simple<char>>(10)
                                        .map(|s| s.parse::<usize>().unwrap()),
                                )
                                .map(|((ident, op), val)| match op {
                                    "===" => Condition::Equality(ident, val),
                                    ">" => Condition::Greater(ident, val),
                                    _ => unreachable!(),
                                })),
                    )
                    .then_ignore(just(")")),
            )
            .padded();

        // Yields a Field.
        let statement = assignment
            .or_not()
            .then(reader_call)
            .then_ignore(semi.or_not())
            .map(|(name, kind)| Field { kind, name });

        let conditional_block = if_stmt
            .then(pblock.or(statement.clone().map(|f| vec![f])))
            .map(|(cond, fields)| Field {
                kind: Kind::If(cond, fields),
                name: None,
            });

        let const_rdr_stmt = just("const reader").then(take_until(just(';'))).padded();
        let const_stmt = just("const")
            .padded()
            .then_ignore(ident.padded())
            // .then_ignore(just('=').padded())
            // .then_ignore(just("new").or(just("{}")).padded())
            // HACK: let's hope this is always one line
            .then(take_until(just('\n')))
            .padded();
        let return_stmt = just("return").padded().then(ident.padded()).then(semi);

        open_block
            .ignore_then(const_rdr_stmt.or_not())
            .ignore_then(const_stmt.or_not())
            .ignore_then(choice((conditional_block, statement)).padded().repeated())
            .then_ignore(return_stmt.or_not())
            .then_ignore(close_block.padded())
    });

    let read = just("export function read(")
        .ignore_then(take_until(just(')')))
        .padded()
        .ignore_then(parse_block);

    let name = just("export const name =")
        .padded()
        .ignore_then(string)
        .then_ignore(semi.or_not());
    let opcode = just("export const opcode =")
        .padded()
        .ignore_then(text::int::<_, Simple<char>>(10))
        .try_map(|s, span| {
            s.parse::<u16>()
                .map_err(|e| Simple::custom(span, format!("{}", e)))
        })
        .then_ignore(semi.or_not());

    // Packet name here is ignored. It's the same as the name given by the export
    // type block, and we can tell whether it's a packet definition or not by
    // whether or not it has an opcode given by this.
    let metadata = name.then(opcode).map(|(_name, opcode)| opcode);

    comment
        .ignore_then(import.repeated())
        .ignore_then(export)
        .then(read)
        .then(metadata.or_not())
        .map(|((name, fields), opcode)| Packet {
            name,
            fields,
            opcode,
        })
}
