#![allow(non_snake_case)]
use alloc::boxed::Box;
use core::{
    cell::UnsafeCell,
    pin::Pin,
    time::Duration,
};

use kdef::_KTIMER;
use lazy_link::lazy_link;
use winapi::shared::ntdef::PVOID;

use crate::Waitable;

#[lazy_link(resolver = "kapi_kmodule::resolve_import")]
extern "C" {
    pub fn KeInitializeTimer(Timer: *mut _KTIMER);
    pub fn KeCancelTimer(Timer: *mut _KTIMER) -> bool;
    pub fn KeSetTimerEx(Timer: *mut _KTIMER, DueTime: i64, Period: i32, _: *mut ()) -> bool;
}

pub struct KTimer {
    inner: Pin<Box<UnsafeCell<_KTIMER>>>,
}

impl KTimer {
    pub fn new() -> Self {
        let inner = Box::pin(UnsafeCell::new(unsafe { core::mem::zeroed() }));

        unsafe { KeInitializeTimer(&mut *inner.get()) };
        Self { inner }
    }

    pub fn set(&self, duration: Duration) -> bool {
        unsafe {
            KeSetTimerEx(
                &mut *self.inner.get(),
                (duration.as_nanos() / 100) as i64 * -1,
                0,
                core::ptr::null_mut(),
            )
        }
    }

    // TODO: Only works with callbacks and can not be waited on.
    // /// Set a repeating interval.
    // /// Please note, that this has at best a millisecond precition
    // pub fn set_repeating(&self, interval: Duration) -> bool {
    //     debug_assert!(interval.as_millis() > 0);
    //     let imports = TIMER_IMPORTS.unwrap();

    //     let initial_period = (interval.as_nanos() / 100) as i64 * -1;
    //     unsafe {
    //         (imports.KeSetTimerEx)(
    //             &mut *self.inner.get(),
    //             initial_period,
    //             interval.as_millis() as i32,
    //             core::ptr::null_mut()
    //         )
    //     }
    // }

    pub fn clear(&self) -> bool {
        unsafe { KeCancelTimer(&mut *self.inner.get()) }
    }
}

impl Drop for KTimer {
    fn drop(&mut self) {
        self.clear();
    }
}

impl Waitable for KTimer {
    fn waitable(&self) -> &dyn Waitable {
        self
    }

    fn wait_object(&self) -> PVOID {
        self.inner.get() as PVOID
    }
}
