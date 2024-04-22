//! LoA packet parser.

use anyhow::Context;

use crate::definitions::{
    MoveOptionData, SkillMoveOptionData, SkillOptionData, TripodIndex, TripodLevel,
};

pub trait Packet {
    const OPCODE: crate::generated::opcode::Opcode;
}

pub type BumpVec<'bump, T> = Vec<T, &'bump bumpalo::Bump>;

pub fn serialize_bumpvec<T, S>(t: &BumpVec<T>, s: S) -> Result<S::Ok, S::Error>
where
    T: serde::Serialize,
    S: serde::Serializer,
{
    use serde::Serialize;
    t.serialize(s)
}

/// `Parser` is responsible for parsing the payload of a LoA packet into a
/// corresponding Rust structure (see [`crate::packet`]).
pub struct Parser<'a>(&'a [u8]);

impl<'a> Parser<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self(data)
    }

    pub fn raw(&self) -> &[u8] {
        self.0
    }

    pub fn advance(&mut self, count: usize) {
        self.0 = &self.0[count..];
    }

    pub fn skip(&mut self, count: usize) -> anyhow::Result<()> {
        if self.0.len() < count {
            anyhow::bail!("not enough bytes remaining to skip {} bytes", count);
        }
        self.0 = &self.0[count..];
        Ok(())
    }

    pub fn read_u8(&mut self) -> anyhow::Result<u8> {
        let ret = self
            .0
            .get(0)
            .copied()
            .context("not enough bytes remaining to read u8")?;
        self.advance(1);
        Ok(ret)
    }

    pub fn read_u16(&mut self) -> anyhow::Result<u16> {
        let bytes = self
            .0
            .get(0..2)
            .context("not enough bytes remaining to read u16")?;
        let ret = u16::from_ne_bytes(bytes.try_into()?);
        self.advance(2);
        Ok(ret)
    }

    pub fn read_u32(&mut self) -> anyhow::Result<u32> {
        let bytes = self
            .0
            .get(0..4)
            .context("not enough bytes remaining to read u32")?;
        let ret = u32::from_ne_bytes(bytes.try_into()?);
        self.advance(4);
        Ok(ret)
    }

    pub fn read_u64(&mut self) -> anyhow::Result<u64> {
        let bytes = self
            .0
            .get(0..8)
            .context("not enough bytes remaining to read u64")?;
        let ret = u64::from_ne_bytes(bytes.try_into()?);
        self.advance(8);
        Ok(ret)
    }

    pub fn read_i8(&mut self) -> anyhow::Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_i16(&mut self) -> anyhow::Result<i16> {
        let bytes = self
            .0
            .get(0..2)
            .context("not enough bytes remaining to read i16")?;
        let ret = i16::from_ne_bytes(bytes.try_into()?);
        self.advance(2);
        Ok(ret)
    }

    pub fn read_i32(&mut self) -> anyhow::Result<i32> {
        let bytes = self
            .0
            .get(0..4)
            .context("not enough bytes remaining to read i32")?;
        let ret = i32::from_ne_bytes(bytes.try_into()?);
        self.advance(4);
        Ok(ret)
    }

    pub fn read_i64(&mut self) -> anyhow::Result<i64> {
        let bytes = self
            .0
            .get(0..8)
            .context("not enough bytes remaining to read i64")?;
        let ret = i64::from_ne_bytes(bytes.try_into()?);
        self.advance(8);
        Ok(ret)
    }

    pub fn read_f32(&mut self) -> anyhow::Result<f32> {
        let bytes = self
            .0
            .get(0..4)
            .context("not enough bytes remaining to read f32")?;
        let ret = f32::from_ne_bytes(bytes.try_into()?);
        self.advance(4);
        Ok(ret)
    }

    pub fn read_bool(&mut self) -> anyhow::Result<bool> {
        Ok(self.read_u8()? == 1)
    }

    // Parsing routines for various static packet structures follow.

    pub fn read_packed_i64(&mut self) -> anyhow::Result<i64> {
        let flags = self.read_u8()?;
        let sign = (flags as i64) & 1;
        let len = (flags as usize >> 1) & 7;
        let lower = (flags as i64) >> 4;
        let mut ret = 0;
        for i in 0..len {
            ret += (self.read_u8()? as i64) << (8 * i);
        }
        ret = (ret << 4) | lower;
        ret *= sign * (-2) + 1;
        Ok(ret)
    }

    // "simple"?
    pub fn read_simple_u64(&mut self) -> anyhow::Result<u64> {
        let bytes = self.0.get(0..2).context("read_simple_u64 i64")?;
        // peeking
        let s = u16::from_ne_bytes(bytes.try_into()?);
        if (s & 0xfff) < 0x81f {
            self.read_u64()
        } else {
            self.advance(2);
            Ok(u64::from(s) & 0xfff | 0x11000)
        }
    }

    pub fn read_throwaway_flags(&mut self) -> anyhow::Result<()> {
        let flag = self.read_u8()?;
        for i in 0..6 {
            if ((flag >> i) & 1) != 0 {
                self.read_u32()?;
            }
            if ((flag >> 6) & 1) != 0 {
                let count = self.read_u16()?;
                for _ in 0..count {
                    self.read_u8()?;
                }
            }
        }

        Ok(())
    }

    pub fn read_packed_values(&mut self, sizes: &[usize]) -> anyhow::Result<()> {
        let flag = self.read_u8()?;
        for i in 0..7 {
            if ((flag >> i) & 1) != 0 {
                for _ in 0..sizes[i] {
                    self.read_u8()?;
                }
            }
        }
        Ok(())
    }

    /// Parse a LoA packet list structure into a [`BumpVec<T>`].
    pub fn read_list<'bump, T: Event<'bump>>(
        &mut self,
        bump: &'bump bumpalo::Bump,
    ) -> anyhow::Result<BumpVec<'bump, T::Out>> {
        let mut v = BumpVec::new_in(bump);
        let len = self.read_u16()?;
        for _ in 0..len {
            v.push(T::parse(self, bump)?);
        }
        Ok(v)
    }

    /// Read `len` of type `L` followed by `len` `T`s.
    pub fn read_counted<'bump, T, L>(
        &mut self,
        bump: &'bump bumpalo::Bump,
        // len: usize,
        max_len: usize,
    ) -> anyhow::Result<BumpVec<'bump, T::Out>>
    where
        T: Event<'bump>,
        L: Event<'bump>,
        L::Out: TryInto<usize>,
    {
        let len = L::parse(self, bump)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("can't convert to usize"))?;
        let mut v = BumpVec::new_in(bump);
        if len <= max_len {
            for _ in 0..len {
                v.push(T::parse(self, bump)?);
            }
        }
        Ok(v)
    }

    /// Read `len * mult` bytes if `len <= max_len`.
    pub fn read_bytes<'bump>(
        &mut self,
        _: &'bump bumpalo::Bump,
        len: usize,
        multiplier: usize,
        max_len: usize,
    ) -> anyhow::Result<()> {
        if len <= max_len {
            self.skip(len * multiplier)?;
            // for _ in 0..len * multiplier {
            //     self.read_u8()?;
            // }
        }
        Ok(())
    }

    /// Parse a `bool` followed by conditional `T` into an [`Option<T>`].
    pub fn read_optional<'bump, T: Event<'bump>>(
        &mut self,
        bump: &'bump bumpalo::Bump,
    ) -> anyhow::Result<Option<T::Out>> {
        if self.read_bool()? {
            Ok(Some(<T>::parse(self, bump)?))
        } else {
            Ok(None)
        }
    }

    /// Parse a LoA string (UTF16) into a `&str` with backing memory located in the bump allocation.
    pub fn read_str<'bump>(&mut self, bump: &'bump bumpalo::Bump) -> anyhow::Result<&'bump str> {
        let mut bytes = BumpVec::new_in(bump);
        let mut buf = [0u8; 4];
        let len = self.read_u16()? as usize;
        let byte_slice = &self
            .0
            .get(0..len * 2)
            .context("not enough bytes to read str")?;
        let utf16_slice =
            unsafe { std::slice::from_raw_parts(byte_slice.as_ptr() as *const u16, len) };

        for c in std::char::decode_utf16(utf16_slice.iter().cloned()) {
            let c = c?;
            let s = c.encode_utf8(&mut buf);
            bytes.extend_from_slice(s.as_bytes());
        }

        self.advance(len * 2);

        let (ptr, len, _cap) = bytes.into_raw_parts();
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        Ok(std::str::from_utf8(slice)?)
    }
}

/// Implemented by structures that have a byte representation a [`Parser`] may encounter.
///
/// Most notably includes all packet and subpacket structures in [`crate::packet`].
pub trait Event<'bump>: Sized + 'bump {
    type Out = Self;
    fn parse(parser: &mut Parser, bump: &'bump bumpalo::Bump) -> anyhow::Result<Self::Out>;
}

impl Event<'_> for u64 {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        parser.read_u64()
    }
}

impl Event<'_> for u32 {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        parser.read_u32()
    }
}

impl Event<'_> for u16 {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        parser.read_u16()
    }
}

impl Event<'_> for u8 {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        parser.read_u8()
    }
}

impl Event<'_> for i64 {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        parser.read_i64()
    }
}

impl Event<'_> for i32 {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        parser.read_i32()
    }
}

impl Event<'_> for i16 {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        parser.read_i16()
    }
}

impl Event<'_> for i8 {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        parser.read_i8()
    }
}

impl<'bump, T: Event<'bump, Out = T>, const N: usize> Event<'bump> for [T; N] {
    fn parse(parser: &mut Parser, bump: &'bump bumpalo::Bump) -> anyhow::Result<Self> {
        let mut array = unsafe { std::mem::zeroed::<[T; N]>() };
        for i in 0..N {
            array[i] = T::parse(parser, bump)?;
        }
        Ok(array)
    }
}

impl Event<'_> for SkillOptionData {
    fn parse(parser: &mut Parser, bump: &bumpalo::Bump) -> anyhow::Result<Self> {
        let mut data = Self::default();
        let flag = parser.read_u8()?;
        if (flag & 1) != 0 {
            data.layer_index = Some(parser.read_u8()?);
        }
        if ((flag >> 1) & 1) != 0 {
            data.start_stage_index = Some(parser.read_u8()?);
        }
        if ((flag >> 2) & 1) != 0 {
            data.transit_index = Some(parser.read_u32()?);
        }
        if ((flag >> 3) & 1) != 0 {
            data.stage_start_time = Some(parser.read_u32()?);
        }
        if ((flag >> 4) & 1) != 0 {
            data.farmost_dist = Some(parser.read_u32()?);
        }
        if ((flag >> 5) & 1) != 0 {
            data.tripod_index = Some(TripodIndex::parse(parser, bump)?);
        }
        if ((flag >> 6) & 1) != 0 {
            data.tripod_level = Some(TripodLevel::parse(parser, bump)?);
        }
        Ok(data)
    }
}

impl Event<'_> for SkillMoveOptionData {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        let mut data = Self::default();
        let flag = parser.read_u8()?;
        if (flag & 1) != 0 {
            data.move_time = Some(parser.read_u32()?);
        }
        if ((flag >> 1) & 1) != 0 {
            data.stand_up_time = Some(parser.read_u32()?);
        }
        if ((flag >> 2) & 1) != 0 {
            data.down_time = Some(parser.read_u32()?);
        }
        if ((flag >> 3) & 1) != 0 {
            data.freeze_time = Some(parser.read_u32()?);
        }
        if ((flag >> 4) & 1) != 0 {
            data.move_height = Some(parser.read_u32()?);
        }
        if ((flag >> 5) & 1) != 0 {
            data.farmost_dist = Some(parser.read_u32()?);
        }
        if ((flag >> 6) & 1) != 0 {
            let count = parser.read_u16()?;
            if count <= 6 {
                parser.skip(count.into())?;
            }
        }
        Ok(data)
    }
}

impl Event<'_> for MoveOptionData {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        let mut data = Self::default();
        let flag = parser.read_u8()?;
        if (flag & 1) != 0 {
            data.modifier = Some(parser.read_u8()?);
        }
        if ((flag >> 1) & 1) != 0 {
            data.speed = Some(parser.read_u32()?);
        }
        if ((flag >> 2) & 1) != 0 {
            data.next_pos = Some(parser.read_u64()?);
        }
        if ((flag >> 3) & 1) != 0 {
            parser.read_u32()?;
        }
        if ((flag >> 4) & 1) != 0 {
            let count = parser.read_u16()?;
            if count <= 4 {
                parser.skip(count.into())?;
            }
        }
        if ((flag >> 5) & 1) != 0 {
            let count = parser.read_u16()?;
            if count <= 5 {
                parser.skip(count.into())?;
            }
        }
        if ((flag >> 6) & 1) != 0 {
            let count = parser.read_u16()?;
            if count <= 6 {
                parser.skip(count.into())?;
            }
        }
        Ok(data)
    }
}

impl Event<'_> for TripodIndex {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        Ok(Self {
            first: parser.read_u8()?,
            second: parser.read_u8()?,
            third: parser.read_u8()?,
        })
    }
}

impl Event<'_> for TripodLevel {
    fn parse(parser: &mut Parser, _: &bumpalo::Bump) -> anyhow::Result<Self> {
        Ok(Self {
            first: parser.read_u16()?,
            second: parser.read_u16()?,
            third: parser.read_u16()?,
        })
    }
}

/// Representation of an archetype of common internal packet structures.
///
/// Such structures consist of a `length` (of varying width -- usually u16,
/// sometimes u32 -- thus "kinded") followed by `length * MULT` bytes if `length <= MAX_LEN`,
/// where `MULT` and `MAX_LEN` are constants defined per structure.
///
/// This is a structure and not a [`Parser`] function so it can be used
/// as a generic argument, notably with [`Parser::read_optional`].
pub struct KindedBytes<T, const MULT: usize, const MAX_LEN: usize> {
    phantom: std::marker::PhantomData<*const T>,
}

impl<'bump, T, const MULT: usize, const MAX_LEN: usize> Event<'bump>
    for KindedBytes<T, MULT, MAX_LEN>
where
    T: Event<'bump>,
    T::Out: TryInto<usize>,
{
    /// Not a relevant structure for analysis, so output is discarded.
    type Out = ();

    fn parse(parser: &mut Parser, bump: &'bump bumpalo::Bump) -> anyhow::Result<Self::Out> {
        let len = T::parse(parser, bump)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("can't convert to usize"))?;
        parser.read_bytes(bump, len, MULT, MAX_LEN)?;
        Ok(())
    }
}
