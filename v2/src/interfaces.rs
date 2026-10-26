//! Native APIs run only inside a user command. QA injects Services::interfaces.
use crate::core::Interface;
#[cfg(unix)]
pub fn read() -> Result<Vec<Interface>, String> {
    use std::{
        ffi::CStr,
        net::{Ipv4Addr, Ipv6Addr},
    };
    struct Addresses(*mut libc::ifaddrs);
    impl Drop for Addresses {
        fn drop(&mut self) {
            unsafe { libc::freeifaddrs(self.0) }
        }
    }
    let mut first = std::ptr::null_mut();
    // SAFETY: the OS initializes this owned list; RAII frees it exactly once.
    if unsafe { libc::getifaddrs(&mut first) } != 0 {
        return Err("Cannot read OS interfaces".into());
    }
    let owned = Addresses(first);
    let mut next = owned.0;
    let mut count = 0;
    let mut out = Vec::new();
    while !next.is_null() {
        count += 1;
        if count > 4096 {
            return Err("Too many OS interface records".into());
        }
        // SAFETY: pointers remain valid until the list is freed on scope exit.
        let item = unsafe { &*next };
        next = item.ifa_next;
        if item.ifa_addr.is_null()
            || item.ifa_name.is_null()
            || item.ifa_flags & libc::IFF_UP as u32 == 0
            || item.ifa_flags & libc::IFF_RUNNING as u32 == 0
        {
            continue;
        }
        let (family, address) = match unsafe { (*item.ifa_addr).sa_family as i32 } {
            libc::AF_INET => {
                let a = unsafe { &*item.ifa_addr.cast::<libc::sockaddr_in>() };
                (
                    4,
                    Ipv4Addr::from(a.sin_addr.s_addr.to_ne_bytes()).to_string(),
                )
            }
            libc::AF_INET6 => {
                let a = unsafe { &*item.ifa_addr.cast::<libc::sockaddr_in6>() };
                (6, Ipv6Addr::from(a.sin6_addr.s6_addr).to_string())
            }
            _ => continue,
        };
        let name = unsafe { CStr::from_ptr(item.ifa_name) }
            .to_string_lossy()
            .into_owned();
        if name.len() > 1024 || out.len() >= 512 {
            return Err("OS interface capacity exceeded".into());
        }
        out.push(Interface {
            name,
            address,
            family,
            internal: item.ifa_flags & libc::IFF_LOOPBACK as u32 != 0,
        });
    }
    Ok(out)
}
#[cfg(windows)]
pub fn read() -> Result<Vec<Interface>, String> {
    use std::{
        mem::{align_of, size_of},
        net::{Ipv4Addr, Ipv6Addr},
    };
    use windows_sys::Win32::{
        Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_NO_DATA, ERROR_SUCCESS},
        NetworkManagement::{IpHelper::*, Ndis::IfOperStatusUp},
        Networking::WinSock::*,
    };
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut bytes = 15_000u32;
    for _ in 0..3 {
        if bytes > 1_048_576 {
            return Err("OS adapter buffer exceeds capacity".into());
        }
        let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        let start = buffer.as_mut_ptr() as usize;
        let end = start + buffer.len() * size_of::<usize>();
        let head = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        // SAFETY: buffer is aligned, writable, bounded and remains owned for the walk.
        let code = unsafe {
            GetAdaptersAddresses(AF_UNSPEC as u32, flags, std::ptr::null(), head, &mut bytes)
        };
        if code == ERROR_BUFFER_OVERFLOW {
            continue;
        }
        if code == ERROR_NO_DATA {
            return Ok(Vec::new());
        }
        if code != ERROR_SUCCESS {
            return Err("Cannot read OS adapters".into());
        }
        let valid = |p: usize, n: usize, alignment: usize| {
            p >= start && p.checked_add(n).is_some_and(|x| x <= end) && p % alignment == 0
        };
        let mut next = head;
        let mut out = Vec::new();
        let mut adapters = 0;
        while !next.is_null() {
            if !valid(
                next as usize,
                size_of::<IP_ADAPTER_ADDRESSES_LH>(),
                align_of::<IP_ADAPTER_ADDRESSES_LH>(),
            ) {
                return Err("Invalid OS adapter record".into());
            }
            let adapter = unsafe { &*next };
            next = adapter.Next;
            adapters += 1;
            if adapters > 512 {
                return Err("Too many OS adapters".into());
            }
            if adapter.OperStatus != IfOperStatusUp || adapter.FirstUnicastAddress.is_null() {
                continue;
            }
            let p = adapter.FriendlyName;
            let mut utf16 = Vec::new();
            if p.is_null() {
                return Err("Missing OS adapter name".into());
            }
            for i in 0..1025 {
                let q = unsafe { p.add(i) };
                if !valid(q as usize, 2, 2) {
                    return Err("Invalid OS adapter name".into());
                }
                let c = unsafe { *q };
                if c == 0 {
                    break;
                }
                utf16.push(c);
            }
            if utf16.len() > 1024 {
                return Err("OS adapter name exceeds capacity".into());
            }
            let name = String::from_utf16_lossy(&utf16);
            let mut unicast = adapter.FirstUnicastAddress;
            let mut count = 0;
            while !unicast.is_null() {
                count += 1;
                if count > 512 || out.len() >= 512 {
                    return Err("Too many OS addresses".into());
                }
                if !valid(
                    unicast as usize,
                    size_of::<IP_ADAPTER_UNICAST_ADDRESS_LH>(),
                    align_of::<IP_ADAPTER_UNICAST_ADDRESS_LH>(),
                ) {
                    return Err("Invalid OS unicast record".into());
                }
                let entry = unsafe { &*unicast };
                unicast = entry.Next;
                let p = entry.Address.lpSockaddr;
                if !valid(p as usize, size_of::<SOCKADDR>(), align_of::<SOCKADDR>()) {
                    return Err("Invalid OS address".into());
                }
                let (family, address) = match unsafe { (*p).sa_family } {
                    AF_INET => {
                        if !valid(
                            p as usize,
                            size_of::<SOCKADDR_IN>(),
                            align_of::<SOCKADDR_IN>(),
                        ) {
                            return Err("Invalid OS IPv4".into());
                        }
                        let a = unsafe { &*p.cast::<SOCKADDR_IN>() };
                        (
                            4,
                            Ipv4Addr::from(unsafe { a.sin_addr.S_un.S_addr }.to_ne_bytes())
                                .to_string(),
                        )
                    }
                    AF_INET6 => {
                        if !valid(
                            p as usize,
                            size_of::<SOCKADDR_IN6>(),
                            align_of::<SOCKADDR_IN6>(),
                        ) {
                            return Err("Invalid OS IPv6".into());
                        }
                        let a = unsafe { &*p.cast::<SOCKADDR_IN6>() };
                        (6, Ipv6Addr::from(unsafe { a.sin6_addr.u.Byte }).to_string())
                    }
                    _ => continue,
                };
                out.push(Interface {
                    name: name.clone(),
                    address,
                    family,
                    internal: adapter.IfType == 24,
                });
            }
        }
        return Ok(out);
    }
    Err("OS adapter list changed repeatedly".into())
}
#[cfg(not(any(unix, windows)))]
pub fn read() -> Result<Vec<Interface>, String> {
    Err("OS interface API unavailable".into())
}
