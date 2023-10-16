const KI_USER_SHARED_DATA: u64 = 0xFFFFF78000000000;
const KU_SHARED_OFFSET_TICK_COUNT: u64 = 0x320;

#[allow(non_snake_case)]
pub fn KeQueryTickCount() -> u64 {
    let tick_count = (KI_USER_SHARED_DATA + KU_SHARED_OFFSET_TICK_COUNT) as *const u64;
    unsafe { *tick_count }
}