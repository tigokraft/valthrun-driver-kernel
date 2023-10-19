use alloc::sync::Arc;
use utils_imports::{dynamic_import_table, provider::SystemExport};
use core::cell::SyncUnsafeCell;

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
}

impl Waitable for KEvent {
    fn waitable(&self) -> &dyn Waitable {
        self
    }
    
    fn wait_object(&self) -> PVOID {
        self.kevent() as PVOID
    }
}