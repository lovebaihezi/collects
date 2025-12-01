use crate::{Dep, Reg, State, StateUpdater};

pub trait Compute: State {
    fn compute(&self, deps: Dep, updater: StateUpdater);

    fn deps(&self) -> &'static [Reg] {
        &[]
    }
}
