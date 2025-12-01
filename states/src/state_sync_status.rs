#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateSyncStatus {
    #[default]
    BeforeInit,
    Initialized,
    Pending,
    Dirty,
    Clean,
}
