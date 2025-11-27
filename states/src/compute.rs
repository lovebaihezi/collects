use crate::{Reg, State, StateCtx, StateReader, StateUpdater};

pub trait Compute: State {
    fn compute(&self, ctx: &StateCtx);

    fn deps(&self) -> &'static [Reg] {
        &[]
    }

    fn reader(&self, ctx: &StateCtx) -> StateReader<Self> {
        StateReader::from_runtime(ctx.runtime())
    }

    fn updater(&self, ctx: &StateCtx) -> StateUpdater<Self> {
        crate::StateUpdater::from_runtime(ctx.runtime())
    }
}
