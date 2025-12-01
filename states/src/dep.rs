use std::{any::Any, collections::BTreeMap, ptr::NonNull};

use crate::{Reg, State};

pub struct Dep {
    inner: BTreeMap<Reg, NonNull<dyn Any>>,
}

impl Dep {
    pub fn new(iter: impl Iterator<Item = (Reg, Option<NonNull<dyn Any>>)>) -> Self {
        let mut inner = BTreeMap::new();
        for (reg, opt_ptr) in iter {
            if let Some(ptr) = opt_ptr {
                inner.insert(reg, ptr);
            }
        }
        Self { inner }
    }

    pub fn get_ref<'a, 'b: 'a, T: State>(&'a self, id: Reg) -> &'b T {
        self.inner
            .get(&id)
            .and_then(|ptr| unsafe { ptr.as_ref().downcast_ref::<T>() })
            .unwrap()
    }

    pub fn boxed<T: State>(&self, id: Reg) -> Box<T> {
        self.inner
            .get(&id)
            .map(|ptr| unsafe { Box::from_raw(ptr.cast::<T>().as_ptr()) })
            .unwrap()
    }
}
