use core::slice;

pub fn search_binary_pattern(address: u64, pattern: &[u8], dummy: u8, direction: i64) -> u64 {
    let mut address = address as i64;
    loop {
        let buffer = unsafe { slice::from_raw_parts(address as *const u8, pattern.len()) };

        let is_match = pattern
            .iter()
            .zip(buffer.iter())
            .find(|(p, v)| **p != dummy && **p != **v)
            .is_none();

        if is_match {
            return address as u64;
        }

        address += direction;
    }
}
