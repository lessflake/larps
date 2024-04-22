//! Wrapper over Win32 WinSock for maintaining a list of `SIO_RCVALL` raw sockets.

use std::{ffi::CStr, net::Ipv4Addr};

use windows_sys::Win32::{Foundation, NetworkManagement::IpHelper, Networking::WinSock};

/// Set of raw sockets mirroring connections made by a given `pid` and `port`
/// between a set of network interface addresses and external endpoints.
pub struct Sockets {
    inner: Vec<RawSocket>,

    pid: u32,
    port: u16,
    addrs: Vec<Ipv4Addr>,
    ips: Vec<Ipv4Addr>,

    fd_set: WinSock::FD_SET,

    // Used in refreshing the set of connections.
    ip_table: IpTable,
    updated_ips: Vec<IpTableEntry>,
    additions: Vec<IpTableEntry>,
    removals: Vec<usize>,

    // RAII wrapper for WSAStartup and WSACleanup.
    _wsa: Wsa,
}

/// Error returned by [`Sockets::select`].
#[derive(Copy, Clone)]
pub enum SelectError {
    WinSock(i32),
    Timeout,
}

impl Sockets {
    /// Count of currently monitored connections.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Create a set of raw sockets listening on connections made by `pid` on external
    /// port `port`. Not populated until [`Self::refresh`] is called.
    pub fn new(pid: u32, port: u16) -> anyhow::Result<Self> {
        let _wsa = Wsa::init()?;

        Ok(Self {
            inner: vec![],
            fd_set: WinSock::FD_SET {
                fd_count: 0,
                fd_array: [0; 64],
            },
            pid,
            port,
            addrs: interfaces()?.filter(|a| !a.is_loopback()).collect(),
            ip_table: IpTable::new()?,
            ips: vec![],
            updated_ips: vec![],
            additions: vec![],
            removals: vec![],
            _wsa,
        })
    }

    /// Select on the set of connections. Blocking with timeout.
    pub fn select(&mut self, timeout: std::time::Duration) -> Result<&[RawSocket], SelectError> {
        let timeout_ms = timeout.as_micros() as i32;
        // select errors if it's given 0 sockets
        if self.inner.len() == 0 {
            std::thread::sleep(timeout);
            return Err(SelectError::Timeout);
        }

        unsafe {
            // Load `fd_set` with our sockets, which are
            // `repr(transparent)` to their underlying FDs.
            self.fd_set.fd_count = self.len() as u32;
            std::ptr::copy_nonoverlapping(
                self.inner.as_ptr() as *const usize,
                self.fd_set.fd_array.as_mut_ptr(),
                self.len(),
            );

            let timeval = WinSock::TIMEVAL {
                tv_sec: timeout_ms / 1_000_000,
                tv_usec: timeout_ms % 1_000_000,
            };

            let ret = WinSock::select(
                0,
                &mut self.fd_set,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &timeval,
            );

            if ret == WinSock::SOCKET_ERROR {
                return Err(SelectError::WinSock(wsa_last_error()));
            }

            if ret == 0 {
                return Err(SelectError::Timeout);
            }

            Ok(std::slice::from_raw_parts(
                self.fd_set.fd_array.as_ptr() as *const RawSocket,
                self.fd_set.fd_count as usize,
            ))
        }
    }

    /// Update monitored connections -- remove dead connections, add any new connections
    /// made since last refresh.
    pub fn refresh(&mut self) -> anyhow::Result<()> {
        // Populate a list of IPs with current TCP connections made by `pid`
        // from external port `port` to one of our network interfaces.
        self.ip_table.refresh()?;
        self.ip_table
            .iter()
            .filter(|e| self.pid == e.pid)
            .filter(|e| self.port == e.dst_port)
            .filter(|e| self.addrs.contains(&e.src_addr))
            .collect_into(&mut self.updated_ips);

        // which IPs do not exist in our current set?
        self.additions.extend(
            self.updated_ips
                .iter()
                .filter(|e| !self.ips.contains(&e.dst_addr))
                .cloned(),
        );

        // which IPs in our current set (by index) do not exist in the updated set?
        self.removals
            .extend(self.ips.iter().enumerate().filter_map(|(i, &ip)| {
                (!self.updated_ips.iter().any(|e| e.dst_addr == ip)).then_some(i)
            }));

        // remove dead connections and add any new connections
        // NOTE: order of operations is significant, as removals are done by index

        // remove highest indexes first so lower indexes do not change
        for removal in self.removals.drain(..).rev() {
            println!("dead connection: {}", self.ips[removal]);
            self.ips.swap_remove(removal);
            self.inner.swap_remove(removal);
        }

        for addition in self.additions.drain(..) {
            println!("new connection: {}", addition.dst_addr);
            self.ips.push(addition.dst_addr);
            self.inner.push(RawSocket::connect(addition)?);
        }

        // NOTE: we're reusing the allocations from `updated_ips`, `removals` and
        //       `additions` every refresh call - they must all start and end the
        //       function empty
        self.updated_ips.clear();

        Ok(())
    }
}

fn make_word(a: u8, b: u8) -> u16 {
    (a as u16) | ((b as u16) << 8)
}

// NOTE: multiple calls to WSACleanup will not perform any cleanup until there has been
//       a call for every successful `WSAStartup`
struct Wsa;

impl Wsa {
    fn init() -> anyhow::Result<Self> {
        unsafe {
            let mut wsa = std::mem::zeroed::<WinSock::WSADATA>();

            match WinSock::WSAStartup(make_word(2, 2), &mut wsa) {
                0 => Ok(Wsa),
                e => Err(anyhow::anyhow!("wsa startup failed; code {e}")),
            }
        }
    }
}

impl Drop for Wsa {
    fn drop(&mut self) {
        unsafe { WinSock::WSACleanup() };
    }
}

/// Wrapper over a Win32 `AF_INET` socket set to `SOCK_RAW` and `SIO_RCVALL`.
#[derive(Debug)]
// Transparent representation required to safely be used in a [`WinSock::FD_SET`] for
// [`WinSock::select`].
#[repr(transparent)]
pub struct RawSocket(WinSock::SOCKET);

impl RawSocket {
    /// Read from the socket.
    pub fn recv(&self, buf: &mut [u8]) -> anyhow::Result<usize> {
        match unsafe {
            WinSock::recvfrom(
                self.0,
                buf.as_mut_ptr(),
                buf.len() as i32,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        } {
            WinSock::SOCKET_ERROR => anyhow::bail!("failed to read; code {}", wsa_last_error()),
            0 => anyhow::bail!("connection closed"),
            len => Ok(len as usize),
        }
    }

    fn connect(conn: IpTableEntry) -> anyhow::Result<Self> {
        let src = SocketAddress::new(conn.src_port, conn.src_addr)?;
        let dst = SocketAddress::new(conn.dst_port, conn.dst_addr)?;
        let socket = Self::init_raw()?;
        socket.bind(src)?;
        socket.set_rcvall()?;
        socket.connect_sys(dst)?;
        Ok(socket)
    }

    fn init_raw() -> anyhow::Result<Self> {
        unsafe {
            let socket = WinSock::socket(
                WinSock::AF_INET.into(),
                WinSock::SOCK_RAW.into(),
                WinSock::IPPROTO_IP as i32,
            );
            if socket == WinSock::INVALID_SOCKET {
                anyhow::bail!("socket creation failed; code {}", wsa_last_error());
            }

            Ok(Self(socket))
        }
    }

    fn bind(&self, addr: SocketAddress) -> anyhow::Result<()> {
        unsafe {
            let ret = WinSock::bind(
                self.0,
                &addr.raw() as *const _ as _,
                std::mem::size_of::<WinSock::SOCKADDR_IN>() as _,
            );
            match ret {
                WinSock::SOCKET_ERROR => anyhow::bail!("bind failed; code {}", wsa_last_error()),
                _ => Ok(()),
            }
        }
    }

    fn ioctl(&self, cmd: i32, arg: &mut u32) -> anyhow::Result<()> {
        unsafe {
            let ret = WinSock::ioctlsocket(self.0, cmd, arg);
            match ret {
                WinSock::SOCKET_ERROR => {
                    anyhow::bail!("sio_rcvall ioctl failed; code {}", wsa_last_error())
                }
                _ => Ok(()),
            }
        }
    }

    fn set_rcvall(&self) -> anyhow::Result<()> {
        self.ioctl(WinSock::SIO_RCVALL as i32, &mut (WinSock::RCVALL_ON as u32))
    }

    fn connect_sys(&self, addr: SocketAddress) -> anyhow::Result<()> {
        unsafe {
            let ret = WinSock::WSAConnect(
                self.0,
                &addr.raw() as *const _ as _,
                std::mem::size_of::<WinSock::SOCKADDR_IN>() as i32,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            );
            match ret {
                WinSock::SOCKET_ERROR => {
                    anyhow::bail!("WSAConnect errored with {}", wsa_last_error())
                }
                _ => Ok(()),
            }
        }
    }
}

impl Drop for RawSocket {
    fn drop(&mut self) {
        unsafe { WinSock::closesocket(self.0) };
    }
}

fn interfaces() -> anyhow::Result<impl Iterator<Item = Ipv4Addr>> {
    // TODO: use `getAdapterAddresses` instead -- `gethostbyname` is deprecated
    unsafe {
        let mut hostname_buf = vec![0u8; 256];
        let ret = WinSock::gethostname(hostname_buf.as_mut_ptr(), hostname_buf.len() as _);
        if ret == WinSock::SOCKET_ERROR {
            anyhow::bail!("failed gethostname");
        }

        let hostname = CStr::from_bytes_until_nul(&hostname_buf).unwrap();
        let hostnames = WinSock::gethostbyname(hostname.as_ptr() as _);

        let ptr = (*hostnames).h_addr_list;
        let mut i = 0;
        Ok(std::iter::from_fn(move || {
            let cur = ptr.add(i);
            if cur.is_null() || (*cur).is_null() {
                return None;
            }
            i += 1;
            Some(Address(*((*cur) as *const WinSock::IN_ADDR)).into_ipv4_addr())
        }))
    }
}

#[derive(Copy, Clone)]
struct Address(WinSock::IN_ADDR);

impl Address {
    fn into_ipv4_addr(self) -> Ipv4Addr {
        unsafe {
            let WinSock::IN_ADDR_0_0 {
                s_b1,
                s_b2,
                s_b3,
                s_b4,
            } = self.0.S_un.S_un_b;
            Ipv4Addr::new(s_b1, s_b2, s_b3, s_b4)
        }
    }
}

#[derive(Copy, Clone)]
struct SocketAddress {
    port: u16,
    addr: Address,
}

impl SocketAddress {
    fn new(port: u16, address: Ipv4Addr) -> anyhow::Result<Self> {
        let [a, b, c, d] = address.octets();
        let addr = WinSock::IN_ADDR {
            S_un: WinSock::IN_ADDR_0 {
                S_un_b: WinSock::IN_ADDR_0_0 {
                    s_b1: a,
                    s_b2: b,
                    s_b3: c,
                    s_b4: d,
                },
            },
        };
        let addr = Address(addr);

        Ok(Self { port, addr })
    }

    // pub fn from_address(port: u16, addr: Address) -> Self {
    //     Self { port, addr }
    // }

    fn raw(&self) -> WinSock::SOCKADDR_IN {
        WinSock::SOCKADDR_IN {
            sin_family: WinSock::AF_INET.into(),
            sin_port: self.port,
            sin_addr: self.addr.0,
            sin_zero: [0; 8],
        }
    }

    fn as_str(&self) -> &str {
        unsafe {
            let ptr_str = WinSock::inet_ntoa(self.addr.0);
            let addr = CStr::from_ptr(ptr_str as *const i8);
            addr.to_str().unwrap()
        }
    }
}

impl std::fmt::Display for SocketAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::fmt::Debug for SocketAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug)]
struct IpTable(Vec<u8>);

impl IpTable {
    fn get_tcp_table_sys(size: &mut u32, ptr: *mut std::ffi::c_void) -> u32 {
        unsafe {
            windows_sys::Win32::NetworkManagement::IpHelper::GetExtendedTcpTable(
                ptr,
                size,
                0,
                WinSock::AF_INET.into(),
                IpHelper::TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        }
    }

    fn new() -> anyhow::Result<Self> {
        let mut size = 0;
        if Self::get_tcp_table_sys(&mut size, std::ptr::null_mut())
            != Foundation::ERROR_INSUFFICIENT_BUFFER
        {
            anyhow::bail!("surprising result from GetTcpTable");
        }

        Ok(Self(vec![0u8; size as usize]))
    }

    fn refresh(&mut self) -> anyhow::Result<()> {
        let mut size = self.0.len() as u32;
        match Self::get_tcp_table_sys(&mut size, self.0.as_mut_ptr() as _) {
            Foundation::NO_ERROR => {}
            Foundation::ERROR_INSUFFICIENT_BUFFER => {
                self.0.resize((size) as usize, 0);
                self.refresh()?;
            }
            _ => anyhow::bail!("GetTcpTable failed; code {}", wsa_last_error()),
        }

        Ok(())
    }

    fn iter(&self) -> impl Iterator<Item = IpTableEntry> {
        unsafe {
            let table: *const IpHelper::MIB_TCPTABLE_OWNER_PID = self.0.as_ptr() as *const _;
            std::slice::from_raw_parts((*table).table.as_ptr(), (*table).dwNumEntries as usize)
                .iter()
                .map(|entry| {
                    let src_addr = Ipv4Addr::from(entry.dwLocalAddr.to_ne_bytes());
                    let src_port = WinSock::ntohs(entry.dwLocalPort.try_into().unwrap());
                    let dst_addr = Ipv4Addr::from(entry.dwRemoteAddr.to_ne_bytes());
                    let dst_port = WinSock::ntohs(entry.dwRemotePort.try_into().unwrap());
                    let pid = entry.dwOwningPid;
                    IpTableEntry {
                        src_addr,
                        src_port,
                        dst_addr,
                        dst_port,
                        pid,
                    }
                })
        }
    }
}

#[derive(Debug, Clone)]
struct IpTableEntry {
    src_addr: Ipv4Addr,
    src_port: u16,
    dst_addr: Ipv4Addr,
    dst_port: u16,
    pid: u32,
}

fn wsa_last_error() -> i32 {
    unsafe { WinSock::WSAGetLastError() }
}
