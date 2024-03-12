use core::marker::PhantomData;

use kapi::{
    LockedMDL,
    Mdl,
    IO_READ_ACCESS,
    IO_WRITE_ACCESS,
};
use vtk_wsk_sys::{
    PMDL,
    _WSK_BUF,
};
use winapi::{
    km::wdm::KPROCESSOR_MODE,
    shared::ntdef::PVOID,
};

use super::{
    WskError,
    WskResult,
};

pub struct WskBuffer<'a> {
    pub buffer: _WSK_BUF,
    locked_mdl: LockedMDL,
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

        let locked_mdl = mdl
            .lock(KPROCESSOR_MODE::KernelMode, access_mode)
            .map_err(|_| WskError::InvalidBuffer)?;

        Ok(Self {
            buffer: _WSK_BUF {
                Mdl: locked_mdl.mdl().raw_mdl() as PMDL,
                Offset: 0,
                Length: length as u64,
            },
            locked_mdl,
            _dummy: Default::default(),
        })
    }

    pub fn size(&self) -> usize {
        self.buffer.Length as usize
    }

    pub unsafe fn into_static(self) -> WskBuffer<'static> {
        WskBuffer {
            buffer: self.buffer,
            locked_mdl: self.locked_mdl,
            _dummy: Default::default(),
        }
    }
}
