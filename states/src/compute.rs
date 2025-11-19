use crate::{Reg, State, StateCtx, StateReader, StateUpdater};

pub trait Compute: State + Sized {
    const TYPE: &'static str = "compute";
    const DEPS: &'static [Reg];

    fn compute(&self, ctx: &StateCtx) -> Option<Self>;

    fn reader(&self, ctx: &StateCtx) -> StateReader<Self> {
        StateReader::from_runtime(ctx.runtime())
    }

    fn updater(&self, ctx: &StateCtx) -> StateUpdater<Self> {
        crate::StateUpdater::from_runtime(ctx.runtime())
    }
}
