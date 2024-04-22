//! Thin FFI wrapper over Oodle compression used by LoA.

use crate::util::{process_path_from_pid, read_snappy_file};

use anyhow::Context;

const OODLE_STATE_LOC: &str = "resources/oodle_state";
const DLL_NAME: &str = "oo2net_9_win64.dll";

pub struct OodleDecompressor {
    fns: OodleDecompressorFns,
    state: Vec<u8>,
    shared: Vec<u8>,
    _window: Vec<u8>,
}

impl OodleDecompressor {
    pub fn init(pid: u32) -> anyhow::Result<Self> {
        let mut path = process_path_from_pid(pid)?;
        path.pop();
        path.push(DLL_NAME);

        unsafe {
            let lib = libloading::Library::new(&path)?;
            let decode_fn: libloading::Symbol<
                unsafe extern "C" fn(*const u8, *const u8, *const u8, isize, *mut u8, isize) -> i32,
            > = lib.get(b"OodleNetwork1UDP_Decode")?;
            let decode_fn = decode_fn.into_raw();
            let state_uncompact_fn: libloading::Symbol<
                unsafe extern "C" fn(*mut u8, *const u8) -> i32,
            > = lib.get(b"OodleNetwork1UDP_State_Uncompact")?;
            let state_uncompact_fn = state_uncompact_fn.into_raw();
            let shared_setwindow_fn: libloading::Symbol<
                unsafe extern "C" fn(*mut u8, i32, *const u8, i32),
            > = lib.get(b"OodleNetwork1_Shared_SetWindow")?;
            let shared_setwindow_fn = shared_setwindow_fn.into_raw();
            let state_size_fn: libloading::Symbol<unsafe extern "C" fn() -> i64> =
                lib.get(b"OodleNetwork1UDP_State_Size")?;
            let state_size_fn = state_size_fn.into_raw();
            let shared_size_fn: libloading::Symbol<unsafe extern "C" fn(i32) -> i64> =
                lib.get(b"OodleNetwork1_Shared_Size")?;
            let shared_size_fn = shared_size_fn.into_raw();
            let fns = OodleDecompressorFns {
                _lib: lib,
                decode_fn,
                state_uncompact_fn,
                shared_setwindow_fn,
                state_size_fn,
                shared_size_fn,
            };

            // Oodle library initialisation
            let payload = read_snappy_file(OODLE_STATE_LOC)?;
            let payload_start = 0x20;
            let window_size = 0x800000;
            let ht_bits = 0x13;
            let compacted_size = i32::from_ne_bytes(payload[0x18..][..4].try_into()?) as usize;
            let compacted_state = &payload[payload_start + window_size..][..compacted_size];
            let mut state = vec![0; fns.state_size() as usize];
            let mut shared = vec![0; fns.shared_size(ht_bits) as usize];
            let window = payload[payload_start..][..window_size].to_vec();

            if !fns.state_uncompact(&mut state, compacted_state) {
                anyhow::bail!("failed to uncompact oodle state");
            }
            fns.shared_setwindow(&mut shared, ht_bits, &window);

            Ok(Self {
                fns,
                state,
                shared,
                _window: window,
            })
        }
    }

    pub fn decompress<'a>(&mut self, buf: &'a mut [u8], data: &[u8]) -> anyhow::Result<&'a [u8]> {
        let len = i32::from_le_bytes(data[..4].try_into()?) as usize;
        if buf.len() < len {
            anyhow::bail!("buffer length isn't big enough: {} vs {}", buf.len(), len);
        }
        self.fns
            .decode(&self.state, &self.shared, &data[4..], &mut buf[..len])
            .then_some(&buf[16..len])
            .context("oodle decompression failed")
    }
}

struct OodleDecompressorFns {
    _lib: libloading::Library,
    decode_fn: libloading::os::windows::Symbol<
        unsafe extern "C" fn(*const u8, *const u8, *const u8, isize, *mut u8, isize) -> i32,
    >,
    state_uncompact_fn:
        libloading::os::windows::Symbol<unsafe extern "C" fn(*mut u8, *const u8) -> i32>,
    shared_setwindow_fn:
        libloading::os::windows::Symbol<unsafe extern "C" fn(*mut u8, i32, *const u8, i32)>,
    state_size_fn: libloading::os::windows::Symbol<unsafe extern "C" fn() -> i64>,
    shared_size_fn: libloading::os::windows::Symbol<unsafe extern "C" fn(i32) -> i64>,
}

impl OodleDecompressorFns {
    fn decode(&self, state: &[u8], shared: &[u8], comp: &[u8], raw: &mut [u8]) -> bool {
        unsafe {
            (self.decode_fn)(
                state.as_ptr(),
                shared.as_ptr(),
                comp.as_ptr(),
                comp.len() as isize,
                raw.as_mut_ptr(),
                raw.len() as isize,
            ) != 0
        }
    }

    fn state_uncompact(&self, state: &mut [u8], compressor_state: &[u8]) -> bool {
        unsafe { (self.state_uncompact_fn)(state.as_mut_ptr(), compressor_state.as_ptr()) != 0 }
    }

    fn shared_setwindow(&self, shared: &mut [u8], len: i32, window: &[u8]) {
        unsafe {
            (self.shared_setwindow_fn)(
                shared.as_mut_ptr(),
                len,
                window.as_ptr(),
                window.len() as i32,
            )
        }
    }

    fn state_size(&self) -> i64 {
        unsafe { (self.state_size_fn)() }
    }

    fn shared_size(&self, bits: i32) -> i64 {
        unsafe { (self.shared_size_fn)(bits) }
    }
}
