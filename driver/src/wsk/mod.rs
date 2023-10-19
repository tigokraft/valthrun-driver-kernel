use core::time::Duration;

use alloc::boxed::Box;

use obfstr::obfstr;
use winapi::{
    km::wdm::{
        DEVICE_OBJECT,
        IO_COMPLETION_ROUTINE_RESULT,
        IRP,
        PIRP, _KWAIT_REASON_DelayExecution, KPROCESSOR_MODE,
    },
    shared::{
        ntdef::{
            NotificationEvent,
            NTSTATUS,
            PVOID,
            UNICODE_STRING,
        },
        ntstatus::STATUS_PENDING,
    },
};

use self::sys::{
    addrinfoexW,
    ADDRESS_FAMILY,
    PADDRINFOEXW,
    PSOCKADDR,
    PWSK_BUF,
    PWSK_CLIENT_DISPATCH,
    PWSK_PROVIDER_BASIC_DISPATCH,
    PWSK_PROVIDER_CONNECTION_DISPATCH,
    PWSK_REGISTRATION,
    PWSK_SOCKET,
    WSK_FLAG_CONNECTION_SOCKET,
    WSK_NO_WAIT,
    _WSK_CLIENT_DISPATCH,
    _WSK_CLIENT_NPI,
    _WSK_PROVIDER_BASIC_DISPATCH,
    _WSK_PROVIDER_CONNECTION_DISPATCH,
    _WSK_PROVIDER_DISPATCH,
    _WSK_PROVIDER_NPI,
    _WSK_REGISTRATION,
};
use crate::imports::GLOBAL_IMPORTS;
use kapi::{
    KEvent,
    NTStatusEx, Waitable,
};
use kdef::IoSetCompletionRoutine;

pub mod sys;

mod imports;
pub use imports::*;

mod error;
pub use error::*;

mod buffer;
pub use buffer::*;

mod address;
pub use address::*;

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

pub struct WskAddressInfo<'a> {
    instance: &'a WskInstance,
    inner: PADDRINFOEXW,
}

impl<'a> WskAddressInfo<'a> {
    pub fn iterate_results(&self) -> impl Iterator<Item = &'a addrinfoexW> {
        let mut current_info = self.inner;
        core::iter::from_fn(move || {
            if current_info.is_null() {
                return None;
            }

            let result = unsafe { &*current_info };
            current_info = result.ai_next;
            return Some(result);
        })
    }
}

impl<'a> Drop for WskAddressInfo<'a> {
    fn drop(&mut self) {
        if self.inner.is_null() {
            /* nothing to do. */
            return;
        }

        unsafe {
            (self.instance.dispatch().WskFreeAddressInfo.unwrap())(
                self.instance.provider.Client,
                self.inner,
            );
        }
    }
}

pub struct WskInstance {
    registration: WskRegistration,
    provider: core::pin::Pin<Box<_WSK_PROVIDER_NPI>>,
}

unsafe impl Send for WskInstance {}
unsafe impl Sync for WskInstance {}

impl WskInstance {
    pub fn create(version: u16) -> WskResult<Self> {
        let wsk_imports = WSK_IMPORTS.resolve().map_err(WskError::ImportError)?;

        let registration = WskRegistration::new(version)?;
        let mut provider = Box::pin(unsafe { core::mem::zeroed() });

        unsafe {
            (wsk_imports.WskCaptureProviderNPI)(
                registration.wsk_registration(),
                WSK_NO_WAIT,
                &mut *provider,
            )
            .ok()
            .map_err(WskError::CaptureProvider)?;
        }

        Ok(Self {
            registration,
            provider,
        })
    }

    fn dispatch(&self) -> &_WSK_PROVIDER_DISPATCH {
        unsafe { &*self.provider.Dispatch }
    }

    pub fn create_connection_socket(
        &self,
        family: ADDRESS_FAMILY,
        socket_type: u16,
        protocol: u32,
    ) -> WskResult<WskConnectionSocket> {
        let socket = self.create_socket(
            family,
            socket_type,
            protocol,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            WSK_FLAG_CONNECTION_SOCKET,
        )?;

        Ok(WskConnectionSocket { socket })
    }

    fn create_socket(
        &self,
        family: ADDRESS_FAMILY,
        socket_type: u16,
        protocol: u32,
        socket_context: sys::PVOID,
        dispatch: sys::PVOID,
        flags: u32,
    ) -> WskResult<WskBasicSocket> {
        let context = SyncWskContext::allocate()?;
        let mut status = unsafe {
            (self.dispatch().WskSocket.unwrap())(
                self.provider.Client,
                family,
                socket_type,
                protocol,
                flags,
                socket_context,        /* PVOID SocketContext */
                dispatch,              /* const VOID *Dispatch, */
                core::ptr::null_mut(), /* PEPROCESS OwningProcess, */
                core::ptr::null_mut(), /* PETHREAD OwningThread, */
                core::ptr::null_mut(), /* PSECURITY_DESCRIPTOR SecurityDescriptor, */
                context.irp,
            )
        };

        if status == STATUS_PENDING {
            context.await_event(None);
            status = context.io_status();
        }

        status.ok().map_err(WskError::SocketCreation)?;

        Ok(WskBasicSocket {
            inner: context.io_information() as PWSK_SOCKET,
        })
    }

    #[allow(unused)]
    pub fn get_address_info(
        &self,
        node_name: Option<&UNICODE_STRING>,
        service_name: Option<&UNICODE_STRING>,
    ) -> WskResult<WskAddressInfo> {
        let context = SyncWskContext::allocate()?;
        let mut query_result: PADDRINFOEXW = unsafe { core::mem::zeroed() };

        let status = unsafe {
            (self.dispatch().WskGetAddressInfo.unwrap())(
                self.provider.Client,
                node_name.map_or(core::ptr::null_mut(), |value: &UNICODE_STRING| {
                    value as *const _ as *mut _
                }),
                service_name.map_or(core::ptr::null_mut(), |value: &UNICODE_STRING| {
                    value as *const _ as *mut _
                }),
                0,                     // NameSpace: NS_ALL
                core::ptr::null_mut(), /* Provider: *mut GUID */
                core::ptr::null_mut(), /* Hints: PADDRINFOEXW */
                &mut query_result,
                core::ptr::null_mut(), /* OwningProcess: PEPROCESS */
                core::ptr::null_mut(), /* OwningThread: PETHREAD */
                context.irp,           /* Irp: PIRP */
            )
        };

        if status == STATUS_PENDING {
            if !context.await_event(None) {
                let imports = GLOBAL_IMPORTS.unwrap();

                /* timeout hit */
                unsafe { (imports.IoCancelIrp)(context.irp) };
                context.await_event(None);
                return Err(WskError::Timeout);
            }
        }

        if query_result.is_null() {
            return Err(WskError::QueryResultNull);
        }

        Ok(WskAddressInfo {
            instance: self,
            inner: query_result,
        })
    }
}

impl Drop for WskInstance {
    fn drop(&mut self) {
        if let Ok(wsk_imports) = WSK_IMPORTS.resolve() {
            unsafe {
                (wsk_imports.WskReleaseProviderNPI)(self.registration.wsk_registration());
            }
        }
    }
}

struct SyncWskContext {
    irp: PIRP,
    event: Box<KEvent>,
}

extern "system" fn wsk_irp_sync_completion_routine(
    _device: &mut DEVICE_OBJECT,
    _irp: &mut IRP,
    context: PVOID,
) -> IO_COMPLETION_ROUTINE_RESULT {
    let kevent = unsafe { &mut *(context as *mut KEvent) };
    kevent.signal();

    IO_COMPLETION_ROUTINE_RESULT::StopCompletion
}

impl SyncWskContext {
    /// Use the event to wait for the context
    pub fn allocate() -> WskResult<Self> {
        let imports = GLOBAL_IMPORTS.unwrap();
        let mut event = Box::new(KEvent::new(NotificationEvent));

        let irp = unsafe { (imports.IoAllocateIrp)(1, false) };
        if irp.is_null() {
            return Err(WskError::OutOfMemory("allocate irp"));
        }

        unsafe {
            IoSetCompletionRoutine(
                irp,
                Some(wsk_irp_sync_completion_routine),
                &mut *event as *mut _ as PVOID,
                true,
                true,
                true,
            )
        };

        Ok(Self { irp, event })
    }

    pub fn await_event(&self, timeout: Option<Duration>) -> bool {
        self.event.wait_for(_KWAIT_REASON_DelayExecution, KPROCESSOR_MODE::KernelMode, false, timeout)
    }

    pub fn io_status(&self) -> NTSTATUS {
        unsafe { *(&*self.irp).IoStatus.__bindgen_anon_1.Status() }
    }

    pub fn io_information(&self) -> PVOID {
        unsafe { (&*self.irp).IoStatus.Information as PVOID }
    }
}

impl Drop for SyncWskContext {
    fn drop(&mut self) {
        let imports = GLOBAL_IMPORTS.unwrap();
        unsafe {
            (imports.IoFreeIrp)(self.irp);
        }
    }
}

pub struct WskConnectionSocket {
    socket: WskBasicSocket,
}

impl WskConnectionSocket {
    fn socket_connection_dispatch(&self) -> &_WSK_PROVIDER_CONNECTION_DISPATCH {
        unsafe { &*((&*self.socket.wsk_socket()).Dispatch as PWSK_PROVIDER_CONNECTION_DISPATCH) }
    }

    pub fn bind(&mut self, address: PSOCKADDR) -> WskResult<()> {
        let context = SyncWskContext::allocate()?;
        let mut status = unsafe {
            (self.socket_connection_dispatch().WskBind.unwrap())(
                self.socket.wsk_socket(), /* PWSK_SOCKET Socket, */
                address,                  /* PSOCKADDR LocalAddress, */
                0,                        /* ULONG Flags, */
                context.irp,              /* PIRP Irp */
            )
        };
        if status == STATUS_PENDING {
            context.await_event(None);
            status = context.io_status();
        }

        status.ok().map_err(WskError::OperationFailed)
    }

    pub fn connect(&mut self, address: PSOCKADDR) -> WskResult<()> {
        let context = SyncWskContext::allocate()?;
        let mut status = unsafe {
            (self.socket_connection_dispatch().WskConnect.unwrap())(
                self.socket.wsk_socket(), /* PWSK_SOCKET Socket, */
                address,                  /* PSOCKADDR RemoteAddress, */
                0,                        /* ULONG Flags, */
                context.irp,              /* PIRP Irp */
            )
        };
        if status == STATUS_PENDING {
            context.await_event(None);
            status = context.io_status();
        }

        status.ok().map_err(WskError::OperationFailed)
    }

    pub fn receive(&mut self, buffer: &mut WskBuffer, flags: u32) -> WskResult<usize> {
        let context = SyncWskContext::allocate()?;
        let mut status = unsafe {
            (self.socket_connection_dispatch().WskReceive.unwrap())(
                self.socket.wsk_socket(), /* PWSK_SOCKET Socket, */
                &mut buffer.buffer,       /* PWSK_BUF Buffer, */
                flags,                    /* ULONG Flags, */
                context.irp,              /* PIRP Irp */
            )
        };
        if status == STATUS_PENDING {
            context.await_event(None);
            status = context.io_status();
        }

        status.ok().map_err(WskError::OperationFailed)?;

        let bytes_received = context.io_information() as usize;
        Ok(bytes_received)
    }

    pub fn send(&mut self, buffer: &WskBuffer, flags: u32) -> WskResult<usize> {
        let context = SyncWskContext::allocate()?;
        let mut status = unsafe {
            (self.socket_connection_dispatch().WskSend.unwrap())(
                self.socket.wsk_socket(),               /* PWSK_SOCKET Socket, */
                &buffer.buffer as *const _ as PWSK_BUF, /* PWSK_BUF Buffer, */
                flags,                                  /* ULONG Flags, */
                context.irp,                            /* PIRP Irp */
            )
        };
        if status == STATUS_PENDING {
            context.await_event(None);
            status = context.io_status();
        }

        status.ok().map_err(WskError::OperationFailed)?;

        let bytes_send = context.io_information() as usize;
        Ok(bytes_send)
    }
}

pub struct WskBasicSocket {
    inner: PWSK_SOCKET,
}

impl WskBasicSocket {
    pub fn wsk_socket(&self) -> PWSK_SOCKET {
        self.inner
    }

    fn basic_dispatch(&self) -> &_WSK_PROVIDER_BASIC_DISPATCH {
        unsafe { &*((&*self.inner).Dispatch as PWSK_PROVIDER_BASIC_DISPATCH) }
    }

    fn close(&self) -> WskResult<()> {
        let context = SyncWskContext::allocate()?;
        let mut status =
            unsafe { (self.basic_dispatch().WskCloseSocket.unwrap())(self.inner, context.irp) };

        if status == STATUS_PENDING {
            context.await_event(None);
            status = context.io_status();
        }

        status.ok().map_err(WskError::OperationFailed)
    }
}

impl Drop for WskBasicSocket {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            log::warn!("{}: {:#}", obfstr!("Failed to close wsk socket"), error);
        }
    }
}
