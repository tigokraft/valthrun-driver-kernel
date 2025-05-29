use core::{
    slice,
    usize,
};

pub fn search_binary_pattern(
    address: u64,
    limit: Option<usize>,
    pattern: &[u8],
    dummy: u8,
    direction: i64,
) -> Option<u64> {
    let mut address = address as i64;
    for _ in 0..limit.unwrap_or(usize::MAX) {
        let buffer = unsafe { slice::from_raw_parts(address as *const u8, pattern.len()) };

        let is_match = pattern
            .iter()
            .zip(buffer.iter())
            .find(|(p, v)| **p != dummy && **p != **v)
            .is_none();

        if is_match {
            return Some(address as u64);
        }

        address += direction;
    }

    None
}
