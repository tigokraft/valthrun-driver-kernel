use alloc::boxed::Box;

use kapi::NTStatusEx;
use vtk_wsk_sys::{
    PWSK_CLIENT_DISPATCH,
    PWSK_REGISTRATION,
    _WSK_CLIENT_DISPATCH,
    _WSK_CLIENT_NPI,
    _WSK_REGISTRATION,
};

use crate::{
    WskError,
    WskResult,
    WSK_IMPORTS,
};

struct WskRegistrationInner {
    dispatch: _WSK_CLIENT_DISPATCH,
    registration: _WSK_REGISTRATION,
}

pub struct WskRegistration {
    inner: core::pin::Pin<Box<WskRegistrationInner>>,
}

impl WskRegistration {
    pub fn new(version: u16) -> WskResult<Self> {
        let wsk_imports = WSK_IMPORTS.resolve().map_err(WskError::ImportError)?;

        /* The registration needs to be pinned, as the ptr to the registration must not change! */
        let mut inner = Box::pin(WskRegistrationInner {
            dispatch: unsafe { core::mem::zeroed() },
            registration: unsafe { core::mem::zeroed() },
        });
        inner.dispatch.Version = version;

        let mut client: _WSK_CLIENT_NPI = unsafe { core::mem::zeroed() };
        client.ClientContext = core::ptr::null_mut();
        client.Dispatch = &inner.dispatch;

        unsafe {
            (wsk_imports.WskRegister)(&mut client, &mut inner.registration)
                .ok()
                .map_err(WskError::Register)?;
        }

        Ok(Self { inner })
    }

    pub fn wsk_registration(&self) -> PWSK_REGISTRATION {
        &self.inner.registration as *const _ as PWSK_REGISTRATION
    }

    #[allow(unused)]
    pub fn wsk_client_dispatch(&self) -> PWSK_CLIENT_DISPATCH {
        &self.inner.dispatch as *const _ as PWSK_CLIENT_DISPATCH
    }
}

impl Drop for WskRegistration {
    fn drop(&mut self) {
        if let Ok(wsk_imports) = WSK_IMPORTS.resolve() {
            unsafe {
                (wsk_imports.WskDeregister)(self.wsk_registration());
            }
        }
    }
}
