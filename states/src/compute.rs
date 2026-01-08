use std::{
    any::{Any, TypeId, type_name},
    fmt::Debug,
};

use log::debug;
use ustr::Ustr;

use crate::{Dep, SnapshotClone, Updater};

pub type ComputeDeps = (&'static [TypeId], &'static [TypeId]);

/// The `Compute` trait represents a derived state that depends on other states or computes.
///
/// It encapsulates logic to calculate its value based on dependencies.
pub trait Compute: Debug + Any + SnapshotClone {
    /// Performs the computation logic.
    ///
    /// # Arguments
    ///
    /// * `deps` - Access to dependencies (states and other computes).
    /// * `updater` - A mechanism to update the state of the system.
    fn compute(&self, deps: Dep, updater: Updater);

    /// Defines the dependencies of this compute.
    ///
    /// # Returns
    ///
    /// A tuple where:
    /// * `.0` is a slice of `TypeId`s for required `State`s.
    /// * `.1` is a slice of `TypeId`s for required `Compute`s.
    fn deps(&self) -> ComputeDeps;

    /// returns the `Any` trait object for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Assigns a new value to this compute from a boxed `Any`.
    ///
    /// Used for updating the compute's value after a recalculation.
    fn assign_box(&mut self, new_self: Box<dyn Any + Send>);

    /// Returns the name of the compute type.
    ///
    /// Defaults to the type name.
    fn name(&self) -> Ustr {
        Ustr::from(type_name::<Self>())
    }
}

pub fn assign_impl<T: Compute + 'static>(old: &mut T, new: Box<dyn Any + Send>) {
    match new.downcast::<T>() {
        Ok(value) => {
            debug!(
                "Assign New Compute {:?} to Compute {:?}",
                &value,
                type_name::<T>()
            );
            *old = *value;
        }
        Err(_) => {
            panic!(
                "Failed to assign compute: type mismatch, expected {:?}, but any unable to downcast to it",
                type_name::<T>(),
            );
        }
    }
}
