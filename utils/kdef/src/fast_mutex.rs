use winapi::{
    km::wdm::KEVENT,
    shared::ntdef::PVOID,
};

#[repr(C)]
pub struct _FAST_MUTEX {
    pub Count: i32,
    pub Owner: PVOID,
    pub Contention: u32,
    pub Event: KEVENT,
    pub OldIrql: u32,
}
