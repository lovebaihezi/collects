use crate::{Reg, State, StateCtx, StateReader, StateUpdater};

pub trait Compute: State {
    fn compute(&mut self, ctx: &StateCtx);

    fn deps(&self) -> &'static [Reg] {
        &[]
    }
}

pub fn reader<T: Compute>(ctx: &StateCtx) -> StateReader<T> {
    StateReader::from_runtime(ctx.runtime())
}

pub fn updater<T: Compute>(ctx: &StateCtx) -> StateUpdater<T> {
    crate::StateUpdater::from_runtime(ctx.runtime())
}
