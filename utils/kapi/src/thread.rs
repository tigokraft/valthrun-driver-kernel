use alloc::{
    boxed::Box,
    sync::Arc,
};
use core::{
    cell::UnsafeCell,
    time::Duration,
};

use obfstr::obfstr;
use winapi::{
    km::wdm::{
        _KWAIT_REASON_Executive,
        KPROCESSOR_MODE,
    },
    shared::{
        ntdef::PVOID,
        ntstatus::STATUS_SUCCESS,
    },
};

use super::{
    NTStatusEx,
    Object,
};
use crate::imports::{
    KeDelayExecutionThread,
    KeWaitForSingleObject,
    PsCreateSystemThread,
    ZwClose,
};

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

pub enum TryJoinResult<T> {
    Success(T),
    Timeout(JoinHandle<T>),
}

impl<T> JoinHandle<T> {
    pub fn try_join(self, timeout: Duration) -> TryJoinResult<T> {
        let timeout = (timeout.as_nanos() / 100) as i64 * -1;

        let success = unsafe {
            KeWaitForSingleObject(
                self.thread_object.cast(),
                _KWAIT_REASON_Executive as u32,
                KPROCESSOR_MODE::KernelMode,
                false,
                &timeout,
            ) == STATUS_SUCCESS
        };

        if success {
            /* thread has exited, therefore it's save to assume, that only we access it */
            let result = unsafe { &mut *self.result.get() };
            TryJoinResult::Success(result.take().unwrap())
        } else {
            TryJoinResult::Timeout(self)
        }
    }

    pub fn join(self) -> T {
        let success = unsafe {
            KeWaitForSingleObject(
                self.thread_object.cast(),
                _KWAIT_REASON_Executive as u32,
                KPROCESSOR_MODE::KernelMode,
                false,
                core::ptr::null(),
            ) == STATUS_SUCCESS
        };

        if !success {
            panic!("{}", obfstr!("to successfully wait for thread handle"));
        }

        /* thread has exited, therefore it's save to assume, that only we access it */
        let result = unsafe { &mut *self.result.get() };
        result.take().unwrap()
    }
}

extern "C" fn rust_thread_user_callback<F, T>(context: PVOID)
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let context = unsafe { Box::from_raw(context as *mut ThreadContext<F, T>) };
    let result = unsafe { &mut *context.result.get() };

    *result = Some((context.callback)());
}

// #[link(name = "ntoskrnl")]
// extern "system" {
//     fn KeExpandKernelStackAndCalloutEx(
//         Callout: extern "C" fn(PVOID) -> (),
//         Parameter: PVOID,
//         Size: usize,
//         Wait: bool,
//         Context: PVOID
//     ) -> NTSTATUS;
// }
// extern "C" fn rust_thread_start<F, T>(context: PVOID)
// where
//     F: FnOnce() -> T + Send + 'static,
//     T: Send + 'static,
// {
//     unsafe {
//         /* Set the stack size to 8 pages. */
//         KeExpandKernelStackAndCalloutEx(rust_thread_user_callback::<F, T>, context, 0x8000, true, core::ptr::null_mut());
//     }
// }

pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let result = Arc::new(UnsafeCell::new(None));
    let context = Box::new(ThreadContext {
        callback: f,
        result: result.clone(),
    });

    let mut handle = core::ptr::null_mut();
    unsafe {
        PsCreateSystemThread(
            &mut handle,
            0,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            rust_thread_user_callback::<F, T>,
            Box::into_raw(context) as PVOID,
        )
        .ok()
        .expect(obfstr!("threads to always start"))
    }

    let thread_object = Object::reference_by_handle(handle, 0x000F0000 | 0x00100000 | 0xFFFF)
        .ok()
        .expect(obfstr!("the thread object to be present"));

    unsafe { ZwClose(handle) };
    JoinHandle {
        thread_object,
        result,
    }
}

pub fn sleep_ms(time: u64) {
    self::sleep_us(time * 1000);
}

pub fn sleep_us(time: u64) {
    let time = -(time as i64 * 10);
    unsafe {
        KeDelayExecutionThread(KPROCESSOR_MODE::KernelMode, false, &time);
    }
}
