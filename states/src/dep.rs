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
    /// # Deprecation
    ///
    /// **This method is deprecated and restricted to crate-internal use only.**
    ///
    /// Commands must not mutate live state through `Dep`. Instead, commands should:
    /// - Read state from `CommandSnapshot` (owned clones)
    /// - Write updates via queued mechanisms (`Updater::set(...)`, `Updater::set_state(...)`)
    ///
    /// This restriction supports the async-safe command execution model where:
    /// - Commands execute at end-of-frame from a queue
    /// - Commands read only snapshot clones (not live references)
    /// - Multiple async jobs can run concurrently without data races
    ///
    /// See `TODO.md` section 1.4 and `docs/ai/state-model.md` for details.
    ///
    /// # Safety
    /// This is safe only because `Dep` holds exclusive access during compute execution.
    /// Computes are executed synchronously on the UI thread.
    ///
    /// # Panics
    /// Panics if the state type is not registered.
    #[deprecated(
        since = "0.2.0",
        note = "Commands must not mutate live state through Dep. Use CommandSnapshot for reads and Updater for writes."
    )]
    #[allow(clippy::mut_from_ref)]
    #[allow(dead_code)] // Intentionally kept for backwards compatibility but restricted to crate-internal
    pub(crate) fn state_mut<T: State + 'static>(&self) -> &'static mut T {
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

#[cfg(test)]
mod tests {
    use std::any::Any;

    use super::*;
    use crate::{SnapshotClone, state_assign_impl};

    /// A simple test state for Dep tests.
    #[derive(Debug, Clone)]
    struct TestDepState {
        value: i32,
    }

    impl SnapshotClone for TestDepState {
        fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    impl State for TestDepState {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            state_assign_impl(self, new_self);
        }
    }

    #[test]
    fn test_dep_get_state_ref_returns_immutable_reference() {
        // Use Box::leak to get a &'static mut reference that stays valid for the test.
        // This is safe in tests as the memory will be reclaimed when the process exits.
        let state: &'static mut TestDepState = Box::leak(Box::new(TestDepState { value: 42 }));
        let state_ptr = NonNull::new(state as *mut dyn State).unwrap();

        let dep = Dep::new(
            [(TypeId::of::<TestDepState>(), state_ptr)].into_iter(),
            std::iter::empty(),
        );

        let state_ref: &TestDepState = dep.get_state_ref();
        assert_eq!(state_ref.value, 42);
    }

    /// Test documenting that `Dep::state_mut` is restricted to crate-internal use.
    ///
    /// This test exists to document the API restriction:
    /// - `Dep::state_mut` is `pub(crate)` and cannot be accessed by external crates
    /// - This prevents commands from mutating live state through `Dep`
    /// - Commands should use `CommandSnapshot` for reads and `Updater` for writes
    ///
    /// The restriction supports async-safe command execution where:
    /// - Commands execute at end-of-frame from a queue
    /// - Commands read only snapshot clones (not live references)
    /// - Multiple async jobs can run concurrently without data races
    #[test]
    #[allow(deprecated)]
    fn test_dep_state_mut_is_crate_internal_only() {
        // This test verifies that `state_mut` works internally (for legacy support)
        // but documents that it should not be exposed to external crates.
        //
        // External crates cannot call `dep.state_mut::<T>()` because it is pub(crate).
        // This is enforced at compile time by Rust's visibility rules.
        //
        // Use Box::leak to get a &'static mut reference that stays valid for the test.
        // This is safe in tests as the memory will be reclaimed when the process exits.
        let state: &'static mut TestDepState = Box::leak(Box::new(TestDepState { value: 10 }));
        let state_ptr = NonNull::new(state as *mut dyn State).unwrap();

        let dep = Dep::new(
            [(TypeId::of::<TestDepState>(), state_ptr)].into_iter(),
            std::iter::empty(),
        );

        // Internal usage is allowed (for legacy code paths within the crate)
        // but this is deprecated and triggers a warning
        let state_mut: &mut TestDepState = dep.state_mut();
        state_mut.value = 99;

        // Verify the mutation worked
        let state_ref: &TestDepState = dep.get_state_ref();
        assert_eq!(state_ref.value, 99);
    }
}
