//! Generation of the `Opcode` enum and its `from_u16` function.

use std::fmt::Write;

use crate::parse::Packet;

pub fn emit(w: &mut impl Write, packets: &[Packet]) -> anyhow::Result<()> {
    super::emit_notice(w)?;

    w.write_str("#[derive(Debug, Copy, Clone, PartialEq, Eq)]\n")?;
    w.write_str("pub enum Opcode {\n")?;

    for name in packets.iter().filter_map(|p| p.opcode.map(|_| &p.name)) {
        write!(w, "{},\n", &name[3..])?;
    }

    w.write_str("}\n\n")?;

    w.write_str("impl Opcode {\n")?;
    w.write_str("pub const fn from_u16(raw: u16) -> Option<Self> {\n")?;
    w.write_str("Some(match raw {\n")?;

    for (name, opcode) in packets
        .iter()
        .filter_map(|p| p.opcode.map(|o| (&p.name, o)))
    {
        write!(w, "{} => Opcode::{},\n", opcode, &name[3..])?;
    }

    w.write_str("_ => return None,\n")?;
    w.write_str("})\n")?;
    w.write_str("}\n")?;

    w.write_str("pub const fn to_u16(self: Self) -> u16 {\n")?;
    w.write_str("match self {\n")?;
    for (name, opcode) in packets
        .iter()
        .filter_map(|p| p.opcode.map(|o| (&p.name, o)))
    {
        write!(w, "Opcode::{} => {},\n", &name[3..], opcode)?;
    }
    w.write_str("}\n")?;
    w.write_str("}\n")?;

    w.write_str("}\n")?;

    Ok(())
}
