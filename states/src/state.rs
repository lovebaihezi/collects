use std::any::Any;

use crate::{Reg, ctx::StateCtx};

pub trait State: Any {
    const TYPE: &'static str = "state";
    const ID: Reg;

    fn mark_dirty(&self, ctx: &mut StateCtx) {
        ctx.mark_dirty(Self::ID);
    }

    fn mark_pending(&self, ctx: &mut StateCtx) {
        ctx.mark_pending(Self::ID);
    }
}
