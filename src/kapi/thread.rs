use alloc::{
    boxed::Box,
    sync::Arc,
};
use core::cell::UnsafeCell;

use obfstr::obfstr;
use winapi::{
    km::wdm::{
        _KWAIT_REASON_Executive,
        KPROCESSOR_MODE,
    },
    shared::ntdef::PVOID,
};

use super::{
    NTStatusEx,
    Object,
};
use crate::imports::GLOBAL_IMPORTS;

struct ThreadContext<F, T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    result: Arc<UnsafeCell<Option<T>>>,
    callback: F,
}

pub struct JoinHandle<T> {
    thread_object: Object,
    result: Arc<UnsafeCell<Option<T>>>,
}

unsafe impl<T> Send for JoinHandle<T> {}
unsafe impl<T> Sync for JoinHandle<T> {}

impl<T> JoinHandle<T> {
    pub fn join(self) -> T {
        let imports = GLOBAL_IMPORTS.unwrap();
        unsafe {
            (imports.KeWaitForSingleObject)(
                self.thread_object.cast(),
                _KWAIT_REASON_Executive as u32,
                KPROCESSOR_MODE::KernelMode,
                false,
                core::ptr::null(),
            )
            .ok()
            .expect(obfstr!("to successfully wait for thread handle"))
        }

        /* thread has exited, therefore it's save to assume, that only we access it */
        let result = unsafe { &mut *self.result.get() };
        result.take().unwrap()
    }
}

extern "C" fn rust_thread_start<F, T>(context: PVOID)
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let context = unsafe { Box::from_raw(context as *mut ThreadContext<F, T>) };
    let result = unsafe { &mut *context.result.get() };

    *result = Some((context.callback)());
}

pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let imports = GLOBAL_IMPORTS.unwrap();

    let result = Arc::new(UnsafeCell::new(None));
    let context = Box::new(ThreadContext {
        callback: f,
        result: result.clone(),
    });

    let mut handle = core::ptr::null_mut();
    unsafe {
        (imports.PsCreateSystemThread)(
            &mut handle,
            0,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            rust_thread_start::<F, T>,
            Box::into_raw(context) as PVOID,
        )
        .ok()
        .expect(obfstr!("threads to always start"))
    }

    let thread_object = Object::reference_by_handle(handle, 0x000F0000 | 0x00100000 | 0xFFFF)
        .ok()
        .expect(obfstr!("the thread object to be present"));

    unsafe { (imports.ZwClose)(handle) };
    JoinHandle {
        thread_object,
        result,
    }
}

pub fn sleep_ms(time: u64) {
    self::sleep_us(time * 1000);
}

pub fn sleep_us(time: u64) {
    let imports = GLOBAL_IMPORTS.unwrap();
    unsafe {
        (imports.KeDelayExecutionThread)(KPROCESSOR_MODE::KernelMode, false, &time);
    }
}
