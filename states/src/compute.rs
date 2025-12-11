use std::{
    any::{Any, TypeId, type_name},
    fmt::Debug,
};

use log::debug;
use ustr::Ustr;

use crate::{Dep, Updater};

pub type ComputeDeps = (&'static [TypeId], &'static [TypeId]);

#[derive(Debug, PartialEq, Eq)]
pub enum ComputeStage {
    Finished,
    Pending,
}

pub trait Compute: Debug + Any {
    fn compute(&self, deps: Dep, updater: Updater) -> ComputeStage;

    // .0 means states, .1 means computes
    fn deps(&self) -> ComputeDeps;

    fn as_any(&self) -> &dyn Any;

    fn as_boxed_any(self) -> Box<dyn Any>;

    fn assign_box(&mut self, new_self: Box<dyn Any>);

    fn name(&self) -> Ustr {
        Ustr::from(type_name::<Self>())
    }
}

pub fn assign_impl<T: Compute + 'static>(old: &mut T, new: Box<dyn Any>) {
    match new.downcast::<T>() {
        Ok(value) => {
            debug!(
                "Assign New Compute {:?} to Compute {:?}",
                &value,
                type_name::<T>()
            );
            *old = *value;
        }
        Err(any) => {
            // TODO: find way to store the type name
            let id = any.type_id();
            panic!(
                "Failed to assign compute: type mismatch, expected {:?}, found type id {:?}",
                type_name::<T>(),
                id
            );
        }
    }
}
