use std::any::Any;

use crate::{StateID, ctx::StateCtx};

pub trait State: Any {
    const ID: StateID;

    const DEPS: &'static [StateID];

    fn compute(&mut self, _ctx: &StateCtx);

    fn re_compute(&mut self, ctx: &StateCtx) {
        self.compute(ctx);
    }

    fn mark_dirty(&self, ctx: &mut StateCtx) {
        ctx.mark_dirty(Self::ID);
    }

    fn mark_pending(&self, ctx: &mut StateCtx) {
        ctx.mark_pending(Self::ID);
    }
}
