#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateSyncStatus {
    #[default]
    Init,
    Pending,
    Dirty,
    Clean,
}
