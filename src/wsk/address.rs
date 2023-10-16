use alloc::string::{
    String,
    ToString,
};

use obfstr::obfstr;

use super::sys::{
    sockaddr,
    AF_INET,
    AF_INET6,
    SOCKADDR_INET,
};
use crate::{
    imports::GLOBAL_IMPORTS,
    kapi::NTStatusEx,
};

pub trait SocketAddrInetEx {
    fn si_family(&self) -> u16;

    fn port(&self) -> u16;
    fn port_mut(&mut self) -> &mut u16;

    fn to_string(&self) -> String;
    fn as_sockaddr(&self) -> &sockaddr;
    fn as_sockaddr_mut(&mut self) -> &mut sockaddr;
}

impl SocketAddrInetEx for SOCKADDR_INET {
    fn si_family(&self) -> u16 {
        unsafe { self.si_family }
    }

    fn port(&self) -> u16 {
        unsafe {
            match self.si_family as u32 {
                AF_INET => self.Ipv4.sin_port,
                AF_INET6 => self.Ipv6.sin6_port,
                _ => panic!("sockaddr inet family unknown"),
            }
        }
    }

    fn port_mut(&mut self) -> &mut u16 {
        unsafe {
            match self.si_family as u32 {
                AF_INET => &mut self.Ipv4.sin_port,
                AF_INET6 => &mut self.Ipv6.sin6_port,
                _ => panic!("sockaddr inet family unknown"),
            }
        }
    }

    fn to_string(&self) -> String {
        let mut buffer = [0u8; 128];
        let mut buffer_length = buffer.len() as u32;

        let imports = if let Ok(imports) = GLOBAL_IMPORTS.resolve() {
            imports
        } else {
            panic!("{}", obfstr!("global imports should have been resolved"))
        };

        let status = match unsafe { self.si_family } as u32 {
            AF_INET => unsafe {
                (imports.RtlIpv4AddressToStringExA)(
                    &self.Ipv4.sin_addr,
                    self.Ipv4.sin_port,
                    buffer.as_mut_ptr(),
                    &mut buffer_length,
                )
            },
            AF_INET6 => unsafe {
                (imports.RtlIpv6AddressToStringExA)(
                    &self.Ipv6.sin6_addr,
                    self.Ipv6.__bindgen_anon_1.sin6_scope_id,
                    self.Ipv6.sin6_port,
                    buffer.as_mut_ptr(),
                    &mut buffer_length,
                )
            },
            _ => panic!("sockaddr inet family unknown"),
        };

        if !status.is_ok() || buffer_length < 1 {
            return String::new();
        }

        buffer_length -= 1; /* get rid of the null terminator */
        String::from_utf8_lossy(&buffer[0..buffer_length as usize]).to_string()
    }

    fn as_sockaddr(&self) -> &sockaddr {
        unsafe { core::mem::transmute_copy(&self) }
    }

    fn as_sockaddr_mut(&mut self) -> &mut sockaddr {
        unsafe { core::mem::transmute_copy(&self) }
    }
}
