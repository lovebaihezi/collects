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
