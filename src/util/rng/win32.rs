use rand_core::{
    CryptoRng,
    RngCore,
};

use crate::imports::GLOBAL_IMPORTS;

/// Random number generator using RtlRandomEx
pub struct Win32Rng {
    seed: u32,
}

impl Win32Rng {
    pub fn new() -> Self {
        let imports = GLOBAL_IMPORTS.resolve().unwrap();
        let seed = {
            let mut buffer = 0;
            unsafe { (imports.KeQuerySystemTimePrecise)(&mut buffer) };
            buffer as u32
        };
        Self { seed }
    }
}

impl CryptoRng for Win32Rng {}

impl RngCore for Win32Rng {
    fn next_u32(&mut self) -> u32 {
        let imports = GLOBAL_IMPORTS.resolve().unwrap();
        unsafe { (imports.RtlRandomEx)(&mut self.seed) }
    }

    fn next_u64(&mut self) -> u64 {
        (self.next_u32() as u64) << 32 | (self.next_u32() as u64)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for entry in dest.iter_mut() {
            *entry = (self.next_u32() & 0xFF) as u8;
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
