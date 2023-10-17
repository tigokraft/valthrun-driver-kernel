use core::marker::PhantomData;

use winapi::{
    km::wdm::KPROCESSOR_MODE,
    shared::ntdef::PVOID,
};

use super::{
    sys::_WSK_BUF,
    WskError,
    WskResult,
};
use crate::{
    imports::GLOBAL_IMPORTS,
    kapi::mem::{
        self,
        Mdl,
        IO_READ_ACCESS,
        IO_WRITE_ACCESS,
    },
};

pub struct WskBuffer<'a> {
    pub buffer: _WSK_BUF,
    _dummy: PhantomData<&'a ()>,
}

#[allow(unused)]
impl<'a> WskBuffer<'a> {
    pub fn create_ro(buffer: &'a [u8]) -> WskResult<Self> {
        Self::create_internal(buffer.as_ptr() as *mut _, buffer.len(), true)
    }

    pub fn create(buffer: &'a mut [u8]) -> WskResult<Self> {
        Self::create_internal(buffer.as_ptr() as *mut _, buffer.len(), false)
    }

    fn create_internal(address: PVOID, length: usize, read_only: bool) -> WskResult<Self> {
        let mdl = Mdl::allocate(address, length, false, false, core::ptr::null_mut())
            .ok_or(WskError::OutOfMemory("allocate mdl for buffer"))?;

        let access_mode = if read_only {
            IO_READ_ACCESS
        } else {
            IO_WRITE_ACCESS
        };
        if !mem::probe_and_lock_pages(mdl.mdl(), KPROCESSOR_MODE::KernelMode, access_mode) {
            return Err(WskError::InvalidBuffer);
        }

        Ok(Self {
            buffer: _WSK_BUF {
                Mdl: mdl.into_raw(),
                Offset: 0,
                Length: length as u64,
            },
            _dummy: Default::default(),
        })
    }

    pub fn size(&self) -> usize {
        self.buffer.Length as usize
    }

    pub fn into_static(self) -> WskBuffer<'static> {
        WskBuffer {
            buffer: self.buffer,
            _dummy: Default::default(),
        }
    }
}

impl<'a> Drop for WskBuffer<'a> {
    fn drop(&mut self) {
        let imports = GLOBAL_IMPORTS.unwrap();
        unsafe { (imports.MmUnlockPages)(self.buffer.Mdl) };
        Mdl::from_raw(self.buffer.Mdl);
    }
}
