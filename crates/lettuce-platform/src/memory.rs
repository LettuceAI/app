#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySnapshot {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

pub fn system_memory() -> Option<MemorySnapshot> {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    let total_bytes = system.total_memory();
    (total_bytes > 0).then(|| MemorySnapshot {
        total_bytes,
        available_bytes: system.available_memory(),
    })
}
