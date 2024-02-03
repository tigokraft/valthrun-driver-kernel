use alloc::boxed::Box;
use core::{
    cell::UnsafeCell,
    pin::Pin,
    time::Duration,
};

use kdef::_KTIMER;
use utils_imports::{
    dynamic_import_table,
    provider::SystemExport,
};
use winapi::{
    km::wdm::DISPATCHER_HEADER,
    shared::ntdef::{
        LIST_ENTRY,
        PVOID,
        ULARGE_INTEGER,
    },
};

use crate::Waitable;

type KeInitializeTimer = unsafe extern "C" fn(Timer: *mut _KTIMER);
type KeCancelTimer = unsafe extern "C" fn(Timer: *mut _KTIMER) -> bool;
type KeSetTimerEx =
    unsafe extern "C" fn(Timer: *mut _KTIMER, DueTime: i64, Period: i32, *mut ()) -> bool;

dynamic_import_table! {
    pub imports TIMER_IMPORTS {
        pub KeInitializeTimer: KeInitializeTimer = SystemExport::new(obfstr!("KeInitializeTimer")),
        pub KeCancelTimer: KeCancelTimer = SystemExport::new(obfstr!("KeCancelTimer")),
        pub KeSetTimerEx: KeSetTimerEx = SystemExport::new(obfstr!("KeSetTimerEx")),
    }
}

pub struct KTimer {
    inner: Pin<Box<UnsafeCell<_KTIMER>>>,
}

impl KTimer {
    pub fn new() -> Self {
        let imports = TIMER_IMPORTS.unwrap();
        let inner = Box::pin(UnsafeCell::new(unsafe { core::mem::zeroed() }));

        unsafe { (imports.KeInitializeTimer)(&mut *inner.get()) };
        Self { inner }
    }

    pub fn set(&self, duration: Duration) -> bool {
        let imports = TIMER_IMPORTS.unwrap();
        unsafe {
            (imports.KeSetTimerEx)(
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
        let imports = TIMER_IMPORTS.unwrap();
        unsafe { (imports.KeCancelTimer)(&mut *self.inner.get()) }
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
