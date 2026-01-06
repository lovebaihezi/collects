use std::{
    any::{Any, TypeId},
    collections::BTreeMap,
    ptr::NonNull,
};

use crate::{Compute, State};

pub struct Dep {
    inner: BTreeMap<TypeId, NonNull<dyn Any>>,
}

impl Dep {
    pub fn new(
        states: impl Iterator<Item = (TypeId, NonNull<dyn State>)>,
        computes: impl Iterator<Item = (TypeId, NonNull<dyn Compute>)>,
    ) -> Self {
        let mut inner: BTreeMap<TypeId, NonNull<dyn Any>> = BTreeMap::new();
        for (reg, ptr) in states {
            inner.insert(reg, ptr);
        }
        for (reg, ptr) in computes {
            inner.insert(reg, ptr);
        }
        Self { inner }
    }

    pub fn get_state_ref<'a, 'b: 'a, T: State + 'static>(&'a self) -> &'b T {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|ptr| unsafe { ptr.as_ref().downcast_ref::<T>() })
            .unwrap()
    }

    /// Get a mutable reference to a state by type.
    ///
    /// # Safety
    /// This is safe because `Dep` holds exclusive access during command execution.
    /// Commands are dispatched synchronously and have exclusive access to the state context.
    ///
    /// # Panics
    /// Panics if the state type is not registered.
    #[allow(clippy::mut_from_ref)]
    pub fn state_mut<T: State + 'static>(&self) -> &'static mut T {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|ptr| unsafe { ptr.cast::<T>().as_ptr().as_mut() })
            .unwrap()
    }

    pub fn get_compute_ref<'a, 'b: 'a, T: Compute + 'static>(&'a self) -> &'b T {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|ptr| unsafe { ptr.as_ref().downcast_ref::<T>() })
            .unwrap()
    }

    pub fn boxed<T: State>(&self) -> Box<T> {
        self.inner
            .get(&TypeId::of::<T>())
            .map(|ptr| unsafe { Box::from_raw(ptr.cast::<T>().as_ptr()) })
            .unwrap()
    }
}
