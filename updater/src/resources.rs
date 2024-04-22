//! Conversion of resources into desired formats.

use std::{collections::HashMap, fs};

pub trait Resource {
    fn convert(src: Vec<u8>) -> anyhow::Result<Vec<u8>>;
    fn convert_and_write(src: &str, dst: &str) -> anyhow::Result<()> {
        let data = fs::read(src)?;
        let out = Self::convert(data)?;
        fs::write(dst, out)?;
        Ok(())
    }
}

// compress with snap, since that's already used as a dependency in the main project
// so decompression is free

pub struct OodleState;
impl Resource for OodleState {
    fn convert(src: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        Ok(snappy_compress(&src)?)
    }
}

pub struct Xor;
impl Resource for Xor {
    fn convert(src: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        Ok(src)
    }
}

pub struct Skills;
impl Resource for Skills {
    fn convert(src: Vec<u8>) -> anyhow::Result<Vec<u8>> {
        #[derive(Debug, serde::Serialize, serde::Deserialize)]
        struct MdSkill {
            id: u32,
            name: String,
            desc: String,
            classid: u16,
            icon: String,
        }

        #[derive(Debug, serde::Serialize, serde::Deserialize)]
        struct Skill {
            name: String,
            class_id: Option<u16>,
            icon: Option<String>,
        }

        impl From<MdSkill> for Skill {
            fn from(s: MdSkill) -> Self {
                Skill {
                    name: s.name,
                    class_id: (s.classid != 0).then_some(s.classid),
                    icon: (!s.icon.is_empty()).then_some(s.icon),
                }
            }
        }

        let md_skills: HashMap<u32, MdSkill> = serde_json::from_slice(&src)?;
        let skills: HashMap<u32, Skill> = md_skills
            .into_iter()
            .map(|(id, s)| (id, s.into()))
            .collect();

        Ok(snappy_compress(&serde_bare::to_vec(&skills)?)?)
    }
}

fn snappy_compress(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    use std::io::Write as _;
    let mut buf = Vec::new();
    {
        let mut wtr = snap::write::FrameEncoder::new(&mut buf);
        wtr.write_all(&bytes)?;
    }
    Ok(buf)
}
