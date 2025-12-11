#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    #[default]
    BeforeInit,
    Pending,
    Dirty,
    Clean,
}
