/// Shared ceilings for CPU work and resident dictionary data, not preallocations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceBudget {
    /// Maximum concurrent background CPU tasks.
    pub workers: usize,
    /// Admission budget for all resident dictionaries together.
    pub resident_bytes: u64,
}

impl ResourceBudget {
    /// Choose ceilings from total RAM, currently available RAM and usable logical CPUs.
    /// Available-memory headroom also protects a large machine running other applications.
    pub fn detect(total: u64, available: u64, logical_cpus: usize) -> Self {
        const GIB: u64 = 1024 * 1024 * 1024;
        let ceiling = match total / GIB {
            0..=4 => 3 * GIB / 4,
            5..=8 => GIB,
            9..=16 => 4 * GIB,
            17..=32 => 12 * GIB,
            33..=64 => 32 * GIB,
            _ => 64 * GIB,
        };
        let reserve = (total / 6).max(GIB);
        Self {
            workers: logical_cpus.clamp(1, 24).saturating_sub(2).max(1),
            resident_bytes: ceiling.min(available.saturating_sub(reserve)),
        }
    }
}
