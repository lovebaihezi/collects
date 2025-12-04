use std::any::{Any, TypeId};

use crate::{Dep, State, StateUpdater};

pub trait Compute: Any + State {
    fn compute(&self, deps: Dep, updater: StateUpdater);

    fn deps(&self) -> &[TypeId];
}
