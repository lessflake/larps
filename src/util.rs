//! Miscellaneous pid-related and decompression utilities.

use std::{ffi::CStr, path::PathBuf};

use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError},
    System::Threading::{self, OpenProcess, QueryFullProcessImageNameA},
    UI::WindowsAndMessaging,
};

/// Return a list of pids matching a Win32 window class string.
pub fn pids_for_window_class(wc: &[u8]) -> Vec<u32> {
    let mut pids = Vec::new();
    let mut hwnd = 0;

    unsafe {
        loop {
            hwnd = WindowsAndMessaging::FindWindowExA(0, hwnd, wc.as_ptr(), std::ptr::null());
            if hwnd == 0 {
                break;
            }
            let mut pid = 0;
            let _ = WindowsAndMessaging::GetWindowThreadProcessId(hwnd, &mut pid);
            if pid > 0 && !pids.contains(&pid) {
                pids.push(pid);
            }
        }
    }

    pids
}

/// Given a pid, return the path of the executable used to spawn that process.
pub fn process_path_from_pid(pid: u32) -> anyhow::Result<PathBuf> {
    let hproc = unsafe { OpenProcess(Threading::PROCESS_QUERY_LIMITED_INFORMATION, 1, pid) };
    let mut buf = vec![0; 1024];
    let mut len = buf.len() as u32;
    let ret = unsafe { QueryFullProcessImageNameA(hproc, 0, buf.as_mut_ptr(), &mut len) };
    if ret == 0 {
        anyhow::bail!("querying full process image name failed with {}", unsafe {
            GetLastError()
        });
    }
    let ret = unsafe { CloseHandle(hproc) };
    if ret == 0 {
        anyhow::bail!("closing process handle failed with {}", unsafe {
            GetLastError()
        });
    }
    let exe_path = CStr::from_bytes_until_nul(&buf)?.to_str()?;
    Ok(PathBuf::from(exe_path))
}

/// Returns a reader that reads and decompresses the Snappy-encoded file at `path`.
pub fn snappy_file_reader(path: &str) -> anyhow::Result<impl std::io::Read> {
    Ok(snap::read::FrameDecoder::new(std::io::BufReader::new(
        std::fs::File::open(path)?,
    )))
}

/// Reads and decompresses a Snappy-encoded file at `path` into a `Vec<u8>`.
pub fn read_snappy_file(path: &str) -> anyhow::Result<Vec<u8>> {
    let mut payload = Vec::new();
    {
        use std::io::Read as _;
        snappy_file_reader(path)?.read_to_end(&mut payload)?;
    }
    Ok(payload)
}
