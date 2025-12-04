use std::{
    any::{Any, TypeId},
    collections::BTreeMap,
    ptr::NonNull,
};

use crate::State;

pub struct Dep {
    inner: BTreeMap<TypeId, NonNull<dyn Any>>,
}

impl Dep {
    pub fn new(iter: impl Iterator<Item = (TypeId, Option<NonNull<dyn Any>>)>) -> Self {
        let mut inner = BTreeMap::new();
        for (reg, opt_ptr) in iter {
            if let Some(ptr) = opt_ptr {
                inner.insert(reg, ptr);
            }
        }
        Self { inner }
    }

    pub fn get_ref<'a, 'b: 'a, T: State>(&'a self) -> &'b T {
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
