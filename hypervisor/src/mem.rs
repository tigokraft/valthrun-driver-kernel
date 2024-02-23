use alloc::boxed::Box;
use core::arch::asm;

use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};

type MmGetPhysicalAddress = unsafe extern "system" fn(BaseAddress: usize) -> usize;
type MmGetVirtualForPhysical = unsafe extern "system" fn(BaseAddress: usize) -> usize;

dynamic_import_table! {
    imports DYNAMIC_IMPORTS {
        pub MmGetPhysicalAddress: MmGetPhysicalAddress = SystemExport::new("MmGetPhysicalAddress"),
        pub MmGetVirtualForPhysical: MmGetVirtualForPhysical = SystemExport::new("MmGetVirtualForPhysical"),
    }
}

pub trait MemoryAddressEx {
    /// Using MmGetPhysicalAddress to get the physical address for this structure
    fn get_physical_address(&self) -> usize;
}

impl<T> MemoryAddressEx for Box<T> {
    fn get_physical_address(&self) -> usize {
        MemoryAddress::Virtual(&**self as *const _ as usize).physical_address()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MemoryAddress {
    Physical(usize),
    Virtual(usize),
}

#[allow(unused)]
impl MemoryAddress {
    pub fn physical_ptr_mut<T>(self) -> *mut T {
        self.physical_address() as *mut T
    }

    pub fn physical_ptr<T>(self) -> *const T {
        self.physical_address() as *const T
    }

    pub fn virtual_ptr_mut<T>(self) -> *mut T {
        self.virtual_address() as *mut T
    }

    pub fn virtual_ptr<T>(self) -> *const T {
        self.virtual_address() as *const T
    }

    pub fn raw_address(self) -> usize {
        match self {
            Self::Physical(address) => address,
            Self::Virtual(address) => address,
        }
    }

    pub fn physical_address(self) -> usize {
        match self {
            Self::Virtual(_) => self.to_physical().raw_address(),
            Self::Physical(address) => address,
        }
    }

    pub fn virtual_address(self) -> usize {
        match self {
            Self::Physical(_) => self.to_virtual().raw_address(),
            Self::Virtual(address) => address,
        }
    }

    pub fn to_virtual(self) -> Self {
        match self {
            Self::Virtual(address) => Self::Virtual(address),
            Self::Physical(address) => {
                let result = unsafe { (DYNAMIC_IMPORTS.unwrap().MmGetVirtualForPhysical)(address) };
                Self::Virtual(result)
            }
        }
    }

    pub fn to_physical(self) -> Self {
        match self {
            Self::Physical(address) => Self::Physical(address),
            Self::Virtual(address) => {
                let result = unsafe { (DYNAMIC_IMPORTS.unwrap().MmGetPhysicalAddress)(address) };
                Self::Physical(result)
            }
        }
    }
}

#[allow(unused)]
pub enum InvVpidMode {
    IndividualAddress(u16, u64),
    SingleContext(u16),
    AllContext,
    SingleContextRetainGlobal(u16),
}

pub fn invvpid(mode: InvVpidMode) {
    let (t, val) = match mode {
        InvVpidMode::IndividualAddress(vpid, addr) => (0u64, vpid as u128 | ((addr as u128) << 64)),
        InvVpidMode::SingleContext(vpid) => (1u64, vpid as u128),
        InvVpidMode::AllContext => (2u64, 0u128),
        InvVpidMode::SingleContextRetainGlobal(vpid) => (3u64, vpid as u128),
    };

    let _rflags = unsafe {
        let rflags: u64;
        asm!(
            "invvpid {}, [{}]",
            "pushfq",
            "pop {}",
            in(reg) t,
            in(reg) &val,
            lateout(reg) rflags
        );
        rflags
    };
    //error::check_vm_insruction(rflags, "Failed to execute invvpid".into())
}
