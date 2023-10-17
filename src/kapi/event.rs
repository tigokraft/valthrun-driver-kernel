use alloc::sync::Arc;
use core::cell::SyncUnsafeCell;

use winapi::{
    km::wdm::{
        _KWAIT_REASON_Executive,
        IO_PRIORITY::{
            IO_NO_INCREMENT,
            KPRIORITY_BOOST,
        },
        KEVENT,
        KPROCESSOR_MODE,
        PKEVENT,
    },
    shared::{
        ntdef::{
            EVENT_TYPE,
            NTSTATUS,
            PVOID,
        },
        ntstatus::{
            STATUS_ALERTED,
            STATUS_SUCCESS,
            STATUS_TIMEOUT,
        },
    },
    um::winnt::STATUS_USER_APC,
};

use super::NTStatusEx;
use crate::{
    dynamic_import_table,
    imports::KeWaitForSingleObject,
    util::imports::SystemExport,
};

type KeInitializeEvent = unsafe extern "C" fn(Event: PKEVENT, event_type: EVENT_TYPE, state: bool);
type KeSetEvent =
    unsafe extern "C" fn(Event: PKEVENT, Increment: KPRIORITY_BOOST, Wait: bool) -> i32;
type KeReadStateEvent = unsafe extern "C" fn(Event: PKEVENT) -> i32;
type KeResetEvent = unsafe extern "C" fn(Event: PKEVENT) -> i32;
type KeClearEvent = unsafe extern "C" fn(Event: PKEVENT);

dynamic_import_table! {
    pub imports KEVENT_IMPORTS {
        pub KeInitializeEvent: KeInitializeEvent = SystemExport::new(obfstr!("KeInitializeEvent")),
        pub KeSetEvent: KeSetEvent = SystemExport::new(obfstr!("KeSetEvent")),
        pub KeReadStateEvent: KeReadStateEvent = SystemExport::new(obfstr!("KeReadStateEvent")),
        pub KeResetEvent: KeResetEvent = SystemExport::new(obfstr!("KeResetEvent")),
        pub KeClearEvent: KeClearEvent = SystemExport::new(obfstr!("KeClearEvent")),

        pub KeWaitForSingleObject: KeWaitForSingleObject = SystemExport::new(obfstr!("KeWaitForSingleObject")),
    }
}

#[derive(Clone)]
pub struct KEvent {
    inner: Arc<SyncUnsafeCell<KEVENT>>,
}

unsafe impl Sync for KEvent {}
unsafe impl Send for KEvent {}

impl KEvent {
    pub fn new(event_type: EVENT_TYPE) -> Self {
        let imports = KEVENT_IMPORTS.unwrap();
        let inner = Arc::new(SyncUnsafeCell::new(unsafe { core::mem::zeroed() }));
        unsafe {
            (imports.KeInitializeEvent)(inner.get(), event_type, false);
        }

        Self { inner }
    }

    pub fn kevent(&self) -> PKEVENT {
        self.inner.get()
    }

    pub fn signal(&self) -> i32 {
        self.signal_ex(IO_NO_INCREMENT, false)
    }

    pub fn signal_ex(&self, increment: KPRIORITY_BOOST, wait: bool) -> i32 {
        let imports = KEVENT_IMPORTS.unwrap();
        unsafe { (imports.KeSetEvent)(self.inner.get(), increment, wait) }
    }

    pub fn read_state(&self) -> i32 {
        let imports = KEVENT_IMPORTS.unwrap();
        unsafe { (imports.KeReadStateEvent)(self.inner.get()) }
    }

    pub fn reset_event(&self) -> i32 {
        let imports = KEVENT_IMPORTS.unwrap();
        unsafe { (imports.KeResetEvent)(self.inner.get()) }
    }

    pub fn clear_event(&self) {
        let imports = KEVENT_IMPORTS.unwrap();
        unsafe { (imports.KeClearEvent)(self.inner.get()) }
    }

    pub fn wait_for(&self, timeout: Option<u32>) -> bool {
        const STATUS_USER_APC_: NTSTATUS = STATUS_USER_APC as NTSTATUS;

        let imports = KEVENT_IMPORTS.unwrap();
        unsafe {
            match {
                (imports.KeWaitForSingleObject)(
                    self.kevent() as PVOID,
                    _KWAIT_REASON_Executive as u32,
                    KPROCESSOR_MODE::KernelMode,
                    false,
                    if let Some(timeout) = &timeout {
                        timeout as *const _
                    } else {
                        core::ptr::null()
                    },
                )
            } {
                STATUS_SUCCESS => true,
                STATUS_ALERTED | STATUS_USER_APC_ => false,
                STATUS_TIMEOUT => false,
                status => status.is_ok(),
            }
        }
    }
}
