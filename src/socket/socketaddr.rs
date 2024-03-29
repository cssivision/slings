use std::ascii;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::SocketStorage;

pub struct SocketAddr {
    sockaddr: libc::sockaddr_un,
    socklen: libc::socklen_t,
}

struct AsciiEscaped<'a>(&'a [u8]);

enum AddressKind<'a> {
    Unnamed,
    Pathname(&'a Path),
    Abstract(&'a [u8]),
}

fn path_offset(sockaddr: &libc::sockaddr_un) -> usize {
    let base = sockaddr as *const _ as usize;
    let path = &sockaddr.sun_path as *const _ as usize;
    path - base
}

impl SocketAddr {
    fn address(&self) -> AddressKind<'_> {
        let offset = path_offset(&self.sockaddr);
        // Don't underflow in `len` below.
        if (self.socklen as usize) < offset {
            return AddressKind::Unnamed;
        }
        let len = self.socklen as usize - offset;
        let path = unsafe { &*(&self.sockaddr.sun_path as *const [libc::c_char] as *const [u8]) };

        // macOS seems to return a len of 16 and a zeroed sun_path for unnamed addresses
        if len == 0
            || (cfg!(not(any(target_os = "linux", target_os = "android")))
                && self.sockaddr.sun_path[0] == 0)
        {
            AddressKind::Unnamed
        } else if self.sockaddr.sun_path[0] == 0 {
            AddressKind::Abstract(&path[1..len])
        } else {
            AddressKind::Pathname(OsStr::from_bytes(&path[..len - 1]).as_ref())
        }
    }

    pub(crate) fn new<F>(f: F) -> io::Result<SocketAddr>
    where
        F: FnOnce(*mut libc::sockaddr, &mut libc::socklen_t) -> io::Result<libc::c_int>,
    {
        let mut sockaddr = {
            let sockaddr = mem::MaybeUninit::<libc::sockaddr_un>::zeroed();
            unsafe { sockaddr.assume_init() }
        };

        let raw_sockaddr = &mut sockaddr as *mut libc::sockaddr_un as *mut libc::sockaddr;
        let mut socklen = mem::size_of_val(&sockaddr) as libc::socklen_t;

        f(raw_sockaddr, &mut socklen)?;
        Ok(SocketAddr::from_parts(sockaddr, socklen))
    }

    pub(crate) fn from_parts(sockaddr: libc::sockaddr_un, socklen: libc::socklen_t) -> SocketAddr {
        SocketAddr { sockaddr, socklen }
    }

    pub fn is_unnamed(&self) -> bool {
        matches!(self.address(), AddressKind::Unnamed)
    }

    pub fn as_pathname(&self) -> Option<&Path> {
        if let AddressKind::Pathname(path) = self.address() {
            Some(path)
        } else {
            None
        }
    }

    pub fn as_abstract_namespace(&self) -> Option<&[u8]> {
        if let AddressKind::Abstract(path) = self.address() {
            Some(path)
        } else {
            None
        }
    }
}

impl fmt::Debug for SocketAddr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.address() {
            AddressKind::Unnamed => write!(fmt, "(unnamed)"),
            AddressKind::Abstract(name) => write!(fmt, "{} (abstract)", AsciiEscaped(name)),
            AddressKind::Pathname(path) => write!(fmt, "{:?} (pathname)", path),
        }
    }
}

impl<'a> fmt::Display for AsciiEscaped<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "\"")?;
        for byte in self.0.iter().cloned().flat_map(ascii::escape_default) {
            write!(fmt, "{}", byte as char)?;
        }
        write!(fmt, "\"")
    }
}

impl From<SocketStorage> for SocketAddr {
    fn from(ss: SocketStorage) -> Self {
        let mut storage = ss.storage.to_owned();
        let socklen = ss.socklen;
        let storage: *mut libc::sockaddr_storage = &mut storage as *mut _;
        let sockaddr: libc::sockaddr_un = unsafe { *storage.cast() };
        SocketAddr::from_parts(sockaddr, socklen)
    }
}
