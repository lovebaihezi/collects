use crate::ctx::StateCtx;

pub trait State {
    #[inline]
    fn id(&self) -> usize {
        unimplemented!()
    }

    fn compute(&mut self, ctx: &StateCtx) {
        ctx.mark_dirty(self.id());
    }

    fn re_compute(&mut self, ctx: &StateCtx) {
        self.compute(ctx);
    }

    fn mark_dirty(&self, ctx: &StateCtx) {
        ctx.mark_dirty(self.id());
    }

    fn mark_pending(&self, ctx: &StateCtx) {
        ctx.mark_pending(self.id());
    }
}
