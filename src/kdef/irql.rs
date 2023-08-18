use winapi::km::wdm::KIRQL;

#[allow(unused)]
extern "system" {
    pub fn KeGetCurrentIrql() -> KIRQL;
}
