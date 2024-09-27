#![allow(non_snake_case)]

use alloc::sync::Arc;
use core::cell::SyncUnsafeCell;

use lazy_link::lazy_link;
use winapi::{
    km::wdm::{
        IO_PRIORITY::{
            IO_NO_INCREMENT,
            KPRIORITY_BOOST,
        },
        KEVENT,
        PKEVENT,
    },
    shared::ntdef::{
        EVENT_TYPE,
        PVOID,
    },
};

use crate::Waitable;

#[lazy_link(resolver = "kapi_kmodule::resolve_import")]
extern "C" {
    pub fn KeInitializeEvent(Event: PKEVENT, event_type: EVENT_TYPE, state: bool);
    pub fn KeSetEvent(Event: PKEVENT, Increment: KPRIORITY_BOOST, Wait: bool) -> i32;
    pub fn KeReadStateEvent(Event: PKEVENT) -> i32;
    pub fn KeResetEvent(Event: PKEVENT) -> i32;
    pub fn KeClearEvent(Event: PKEVENT);
}

#[derive(Clone)]
pub struct KEvent {
    inner: Arc<SyncUnsafeCell<KEVENT>>,
}

unsafe impl Sync for KEvent {}
unsafe impl Send for KEvent {}

impl KEvent {
    pub fn new(event_type: EVENT_TYPE) -> Self {
        let inner = Arc::new(SyncUnsafeCell::new(unsafe { core::mem::zeroed() }));
        unsafe {
            KeInitializeEvent(inner.get(), event_type, false);
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
        unsafe { KeSetEvent(self.inner.get(), increment, wait) }
    }

    pub fn read_state(&self) -> i32 {
        unsafe { KeReadStateEvent(self.inner.get()) }
    }

    pub fn reset_event(&self) -> i32 {
        unsafe { KeResetEvent(self.inner.get()) }
    }

    pub fn clear_event(&self) {
        unsafe { KeClearEvent(self.inner.get()) }
    }
}

impl Waitable for KEvent {
    fn waitable(&self) -> &dyn Waitable {
        self
    }

    fn wait_object(&self) -> PVOID {
        self.kevent() as PVOID
    }
}
