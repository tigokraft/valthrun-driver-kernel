#[repr(C)]
#[derive(Clone, Copy)]
pub struct KIDTEntry64 {
    pub offset_low: u16,
    pub selector: u16,
    pub flags: u16,
    pub offset_middle: u16,
    pub offset_high: u32,
}
const _: [(); 0x0C] = [(); size_of::<KIDTEntry64>()];
