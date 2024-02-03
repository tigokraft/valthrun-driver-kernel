#[allow(non_snake_case)]
pub struct _KTIMER {
    Header: DISPATCHER_HEADER,
    DueTime: ULARGE_INTEGER,
    TimerListEntry: LIST_ENTRY,
    Dpc: *mut (), /* _KDPC */
    Processor: u16,
    TimerType: u16,
    Period: u32,
}
