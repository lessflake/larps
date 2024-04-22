//! Responsible for updating data that routinely changes in LoA client updates.
//! This includes packet formats, the XOR cipher key, the Oodle decompression
//! state, and the database of datamined skills.

use updater::{
    emit, parse,
    resources::{OodleState, Resource, Skills, Xor},
};

const TARGET: &str = "updater/meter-core/src/packets/generated";
const SUBDIRS: &[&str] = &["definitions", "structures"];
const PACKET_DST: &str = "src/generated/packet.rs";
const OPCODE_DST: &str = "src/generated/opcode.rs";

const XOR: &str = "updater/meter-data/xor.bin";
const XOR_DST: &str = "src/generated/xor";

const OODLE_STATE: &str = "updater/meter-data/oodle_state.bin";
const OODLE_STATE_DST: &str = "resources/oodle_state";

const SKILL: &str = "updater/meter-data/databases/Skill.json";
const SKILL_DST: &str = "resources/skills";

fn main() -> anyhow::Result<()> {
    let target = std::env::current_dir()?.join(TARGET);
    let packet_files = SUBDIRS
        .into_iter()
        .flat_map(|sd| target.join(sd).read_dir())
        .flatten()
        .flatten()
        .map(|e| e.path());
    let packets = parse::parse_packets(packet_files);
    emit::write_packets(&packets, PACKET_DST)?;
    emit::write_opcodes(&packets, OPCODE_DST)?;

    Skills::convert_and_write(SKILL, SKILL_DST)?;
    OodleState::convert_and_write(OODLE_STATE, OODLE_STATE_DST)?;
    Xor::convert_and_write(XOR, XOR_DST)?;

    Ok(())
}
