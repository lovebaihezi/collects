use egui::{Context, Id, util::id_type_map::SerializableAny};

pub trait Signal {
    type Output: SerializableAny;

    fn id(&self) -> Id;

    fn get(&self, ctx: &Context) -> Option<Self::Output> {
        ctx.memory(|mem| {
            return mem.data.get_temp(self.id());
        })
    }

    fn set(&self, ctx: &Context, value: Self::Output) {
        ctx.memory_mut(|mem| {
            mem.data.insert_temp(self.id(), value);
        });
    }
}

impl<T> dyn Signal<Output = T> + '_
where
    T: SerializableAny,
{
    pub fn clear(&self, ctx: &Context) {
        ctx.memory_mut(|mem| {
            mem.data.remove::<bool>(self.id());
        });
    }
}

pub fn signal_handle_thread() {}
