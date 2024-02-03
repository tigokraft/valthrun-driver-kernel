use winapi::{
    km::wdm::DISPATCHER_HEADER,
    shared::ntdef::{
        LIST_ENTRY,
        ULARGE_INTEGER,
    },
};

#[allow(non_snake_case)]
pub struct _KTIMER {
    pub Header: DISPATCHER_HEADER,
    pub DueTime: ULARGE_INTEGER,
    pub TimerListEntry: LIST_ENTRY,
    pub Dpc: *mut (), /* _KDPC */
    pub Processor: u16,
    pub TimerType: u16,
    pub Period: u32,
}
