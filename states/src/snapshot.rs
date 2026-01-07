//! Snapshot types for safe, async-compatible command execution.
//!
//! This module provides snapshot containers that hold owned clones of state and compute values.
//! Commands use these snapshots instead of borrowing live mutable references, enabling:
//! - Safe concurrent async work (including WASM)
//! - Deterministic end-of-frame command execution
//! - No mid-frame mutation observation
//!
//! # Usage
//!
//! Snapshots are created by the UI thread at flush time (end-of-frame) and passed to commands.
//! Commands read from snapshots and write updates via the `Updater` channel.
//!
//! ```ignore
//! impl Command for MyCommand {
//!     fn run(&self, snap: CommandSnapshot, updater: Updater) {
//!         let my_state: &MyState = snap.state();
//!         // ... read from snapshot, write via updater ...
//!     }
//! }
//! ```

use std::{
    any::{Any, TypeId, type_name},
    collections::BTreeMap,
    fmt::Debug,
};

/// Snapshot of all registered states at a point in time.
///
/// Provides read-only access to owned clones of state values.
/// States are cloned at snapshot creation time, so commands see a consistent view.
#[derive(Debug)]
pub struct StateSnapshot {
    states: BTreeMap<TypeId, Box<dyn Any + Send>>,
}

impl StateSnapshot {
    /// Creates a new state snapshot from an iterator of (TypeId, boxed state) pairs.
    pub fn new(states: impl Iterator<Item = (TypeId, Box<dyn Any + Send>)>) -> Self {
        Self {
            states: states.collect(),
        }
    }

    /// Gets a reference to a state by type.
    ///
    /// # Panics
    /// Panics if the state type is not present in the snapshot.
    pub fn get<T: 'static>(&self) -> &T {
        self.try_get::<T>()
            .unwrap_or_else(|| panic!("State type {} not found in snapshot", type_name::<T>()))
    }

    /// Tries to get a reference to a state by type.
    ///
    /// Returns `None` if the state type is not present in the snapshot.
    pub fn try_get<T: 'static>(&self) -> Option<&T> {
        self.states
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    /// Checks if a state type is present in the snapshot.
    pub fn contains<T: 'static>(&self) -> bool {
        self.states.contains_key(&TypeId::of::<T>())
    }
}

/// Snapshot of all registered computes at a point in time.
///
/// Provides read-only access to owned clones of compute values.
/// Computes are cloned at snapshot creation time, so commands see a consistent view.
#[derive(Debug)]
pub struct ComputeSnapshot {
    computes: BTreeMap<TypeId, Box<dyn Any + Send>>,
}

impl ComputeSnapshot {
    /// Creates a new compute snapshot from an iterator of (TypeId, boxed compute) pairs.
    pub fn new(computes: impl Iterator<Item = (TypeId, Box<dyn Any + Send>)>) -> Self {
        Self {
            computes: computes.collect(),
        }
    }

    /// Gets a reference to a compute by type.
    ///
    /// # Panics
    /// Panics if the compute type is not present in the snapshot.
    pub fn get<T: 'static>(&self) -> &T {
        self.try_get::<T>()
            .unwrap_or_else(|| panic!("Compute type {} not found in snapshot", type_name::<T>()))
    }

    /// Tries to get a reference to a compute by type.
    ///
    /// Returns `None` if the compute type is not present in the snapshot.
    pub fn try_get<T: 'static>(&self) -> Option<&T> {
        self.computes
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    /// Checks if a compute type is present in the snapshot.
    pub fn contains<T: 'static>(&self) -> bool {
        self.computes.contains_key(&TypeId::of::<T>())
    }
}

/// Combined snapshot of states and computes for command execution.
///
/// This is the primary interface commands use to read data. It wraps both
/// `StateSnapshot` and `ComputeSnapshot` and provides convenient accessor methods.
///
/// # Example
///
/// ```ignore
/// impl Command for FetchDataCommand {
///     fn run(&self, snap: CommandSnapshot, updater: Updater) {
///         let config: &ApiConfig = snap.state();
///         let auth: &AuthCompute = snap.compute();
///         
///         // Use config and auth to make API call...
///         // Send results via updater.set(...)
///     }
/// }
/// ```
#[derive(Debug)]
pub struct CommandSnapshot {
    states: StateSnapshot,
    computes: ComputeSnapshot,
}

impl CommandSnapshot {
    /// Creates a new command snapshot from state and compute snapshots.
    pub fn new(states: StateSnapshot, computes: ComputeSnapshot) -> Self {
        Self { states, computes }
    }

    /// Creates a command snapshot directly from iterators.
    pub fn from_iters(
        states: impl Iterator<Item = (TypeId, Box<dyn Any + Send>)>,
        computes: impl Iterator<Item = (TypeId, Box<dyn Any + Send>)>,
    ) -> Self {
        Self {
            states: StateSnapshot::new(states),
            computes: ComputeSnapshot::new(computes),
        }
    }

    /// Gets a reference to a state by type.
    ///
    /// # Panics
    /// Panics if the state type is not present in the snapshot.
    pub fn state<T: 'static>(&self) -> &T {
        self.states.get::<T>()
    }

    /// Tries to get a reference to a state by type.
    ///
    /// Returns `None` if the state type is not present in the snapshot.
    pub fn try_state<T: 'static>(&self) -> Option<&T> {
        self.states.try_get::<T>()
    }

    /// Gets a reference to a compute by type.
    ///
    /// # Panics
    /// Panics if the compute type is not present in the snapshot.
    pub fn compute<T: 'static>(&self) -> &T {
        self.computes.get::<T>()
    }

    /// Tries to get a reference to a compute by type.
    ///
    /// Returns `None` if the compute type is not present in the snapshot.
    pub fn try_compute<T: 'static>(&self) -> Option<&T> {
        self.computes.try_get::<T>()
    }

    /// Checks if a state type is present in the snapshot.
    pub fn has_state<T: 'static>(&self) -> bool {
        self.states.contains::<T>()
    }

    /// Checks if a compute type is present in the snapshot.
    pub fn has_compute<T: 'static>(&self) -> bool {
        self.computes.contains::<T>()
    }

    /// Returns a reference to the underlying state snapshot.
    pub fn states(&self) -> &StateSnapshot {
        &self.states
    }

    /// Returns a reference to the underlying compute snapshot.
    pub fn computes(&self) -> &ComputeSnapshot {
        &self.computes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestState {
        value: i32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct TestCompute {
        result: String,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct OtherState {
        name: String,
    }

    #[test]
    fn state_snapshot_get() {
        let state = TestState { value: 42 };
        let snap = StateSnapshot::new(
            [(
                TypeId::of::<TestState>(),
                Box::new(state.clone()) as Box<dyn Any + Send>,
            )]
            .into_iter(),
        );

        let retrieved: &TestState = snap.get();
        assert_eq!(retrieved.value, 42);
    }

    #[test]
    fn state_snapshot_try_get() {
        let state = TestState { value: 42 };
        let snap = StateSnapshot::new(
            [(
                TypeId::of::<TestState>(),
                Box::new(state.clone()) as Box<dyn Any + Send>,
            )]
            .into_iter(),
        );

        assert!(snap.try_get::<TestState>().is_some());
        assert!(snap.try_get::<OtherState>().is_none());
    }

    #[test]
    fn state_snapshot_contains() {
        let state = TestState { value: 42 };
        let snap = StateSnapshot::new(
            [(
                TypeId::of::<TestState>(),
                Box::new(state.clone()) as Box<dyn Any + Send>,
            )]
            .into_iter(),
        );

        assert!(snap.contains::<TestState>());
        assert!(!snap.contains::<OtherState>());
    }

    #[test]
    #[should_panic(expected = "State type")]
    fn state_snapshot_get_missing_panics() {
        let snap = StateSnapshot::new(std::iter::empty());
        let _: &TestState = snap.get();
    }

    #[test]
    fn compute_snapshot_get() {
        let compute = TestCompute {
            result: "hello".to_string(),
        };
        let snap = ComputeSnapshot::new(
            [(
                TypeId::of::<TestCompute>(),
                Box::new(compute.clone()) as Box<dyn Any + Send>,
            )]
            .into_iter(),
        );

        let retrieved: &TestCompute = snap.get();
        assert_eq!(retrieved.result, "hello");
    }

    #[test]
    fn compute_snapshot_try_get() {
        let compute = TestCompute {
            result: "hello".to_string(),
        };
        let snap = ComputeSnapshot::new(
            [(
                TypeId::of::<TestCompute>(),
                Box::new(compute.clone()) as Box<dyn Any + Send>,
            )]
            .into_iter(),
        );

        assert!(snap.try_get::<TestCompute>().is_some());
        assert!(snap.try_get::<TestState>().is_none());
    }

    #[test]
    fn command_snapshot_accessors() {
        let state = TestState { value: 123 };
        let compute = TestCompute {
            result: "world".to_string(),
        };

        let snap = CommandSnapshot::from_iters(
            [(
                TypeId::of::<TestState>(),
                Box::new(state.clone()) as Box<dyn Any + Send>,
            )]
            .into_iter(),
            [(
                TypeId::of::<TestCompute>(),
                Box::new(compute.clone()) as Box<dyn Any + Send>,
            )]
            .into_iter(),
        );

        let s: &TestState = snap.state();
        assert_eq!(s.value, 123);

        let c: &TestCompute = snap.compute();
        assert_eq!(c.result, "world");
    }

    #[test]
    fn command_snapshot_try_accessors() {
        let state = TestState { value: 123 };
        let snap = CommandSnapshot::from_iters(
            [(
                TypeId::of::<TestState>(),
                Box::new(state.clone()) as Box<dyn Any + Send>,
            )]
            .into_iter(),
            std::iter::empty(),
        );

        assert!(snap.try_state::<TestState>().is_some());
        assert!(snap.try_state::<OtherState>().is_none());
        assert!(snap.try_compute::<TestCompute>().is_none());
    }

    #[test]
    fn command_snapshot_has_methods() {
        let state = TestState { value: 1 };
        let compute = TestCompute {
            result: "x".to_string(),
        };

        let snap = CommandSnapshot::from_iters(
            [(
                TypeId::of::<TestState>(),
                Box::new(state) as Box<dyn Any + Send>,
            )]
            .into_iter(),
            [(
                TypeId::of::<TestCompute>(),
                Box::new(compute) as Box<dyn Any + Send>,
            )]
            .into_iter(),
        );

        assert!(snap.has_state::<TestState>());
        assert!(!snap.has_state::<OtherState>());
        assert!(snap.has_compute::<TestCompute>());
        assert!(!snap.has_compute::<TestState>());
    }

    #[test]
    fn command_snapshot_inner_snapshots() {
        let state = TestState { value: 1 };
        let compute = TestCompute {
            result: "x".to_string(),
        };

        let snap = CommandSnapshot::from_iters(
            [(
                TypeId::of::<TestState>(),
                Box::new(state) as Box<dyn Any + Send>,
            )]
            .into_iter(),
            [(
                TypeId::of::<TestCompute>(),
                Box::new(compute) as Box<dyn Any + Send>,
            )]
            .into_iter(),
        );

        // Can access inner snapshots
        assert!(snap.states().contains::<TestState>());
        assert!(snap.computes().contains::<TestCompute>());
    }
}
