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

/// Trait for types that can be cloned into a snapshot.
///
/// States and Computes that need to be accessed by Commands must implement this trait.
/// The default implementation returns `None`, meaning the type cannot be snapshotted.
/// Implement `clone_boxed` to enable snapshotting for a specific type.
///
/// # Example
///
/// ```ignore
/// impl SnapshotClone for MyState {
///     fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
///         Some(Box::new(self.clone()))
///     }
/// }
/// ```
pub trait SnapshotClone {
    /// Clone this value into a boxed Any for snapshot storage.
    ///
    /// Returns `None` if this type cannot be snapshotted (e.g., contains non-Send types).
    /// Returns `Some(Box<dyn Any + Send>)` if snapshotting is supported.
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        None
    }
}

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

    // ═══════════════════════════════════════════════════════════════════════
    // SEND-SAFE STATE BOUNDARY TESTS
    //
    // These tests verify the Send-safe vs UI-affine state patterns documented
    // in docs/ai/send-safe-state.md
    // ═══════════════════════════════════════════════════════════════════════

    /// A Send-safe state: all fields are Send, implements clone_boxed returning Some.
    #[derive(Debug, Clone, PartialEq)]
    struct SendSafeState {
        value: i32,
        name: String,
    }

    impl SnapshotClone for SendSafeState {
        fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    /// A UI-affine state simulation: uses default SnapshotClone (returns None).
    /// In real code, this would contain non-Send types like egui::TextureHandle.
    #[derive(Debug, Default)]
    #[allow(dead_code)]
    struct UiAffineState {
        value: i32,
        // In real code: pub texture: Option<egui::TextureHandle>,
    }

    // UI-affine states use the default SnapshotClone implementation which returns None
    impl SnapshotClone for UiAffineState {}

    #[test]
    fn send_safe_state_clone_boxed_returns_some() {
        let state = SendSafeState {
            value: 42,
            name: "test".to_string(),
        };

        let boxed = state.clone_boxed();
        assert!(
            boxed.is_some(),
            "Send-safe state should return Some from clone_boxed"
        );

        // Verify the cloned value is correct
        let boxed_value = boxed.unwrap();
        let downcast = boxed_value.downcast::<SendSafeState>().unwrap();
        assert_eq!(*downcast, state);
    }

    #[test]
    fn ui_affine_state_clone_boxed_returns_none() {
        let state = UiAffineState { value: 42 };

        let boxed = state.clone_boxed();
        assert!(
            boxed.is_none(),
            "UI-affine state should return None from clone_boxed (default impl)"
        );
    }

    #[test]
    fn send_safe_state_included_in_snapshot() {
        let state = SendSafeState {
            value: 42,
            name: "test".to_string(),
        };

        // Simulate how StateCtx creates snapshots: only include states where clone_boxed returns Some
        let states: Vec<(TypeId, Box<dyn Any + Send>)> = state
            .clone_boxed()
            .map(|boxed| (TypeId::of::<SendSafeState>(), boxed))
            .into_iter()
            .collect();

        let snap = StateSnapshot::new(states.into_iter());

        assert!(
            snap.contains::<SendSafeState>(),
            "Send-safe state should be included in snapshot"
        );
        assert_eq!(snap.get::<SendSafeState>().value, 42);
    }

    #[test]
    fn ui_affine_state_excluded_from_snapshot() {
        let state = UiAffineState { value: 42 };

        // Simulate how StateCtx creates snapshots: only include states where clone_boxed returns Some
        let states: Vec<(TypeId, Box<dyn Any + Send>)> = state
            .clone_boxed()
            .map(|boxed| (TypeId::of::<UiAffineState>(), boxed))
            .into_iter()
            .collect();

        let snap = StateSnapshot::new(states.into_iter());

        assert!(
            !snap.contains::<UiAffineState>(),
            "UI-affine state should NOT be included in snapshot"
        );
    }

    #[test]
    fn command_snapshot_excludes_ui_affine_states() {
        let send_safe = SendSafeState {
            value: 42,
            name: "test".to_string(),
        };
        let ui_affine = UiAffineState { value: 100 };

        // Simulate StateCtx::create_command_snapshot behavior
        let states: Vec<(TypeId, Box<dyn Any + Send>)> = [
            send_safe
                .clone_boxed()
                .map(|boxed| (TypeId::of::<SendSafeState>(), boxed)),
            ui_affine
                .clone_boxed()
                .map(|boxed| (TypeId::of::<UiAffineState>(), boxed)),
        ]
        .into_iter()
        .flatten()
        .collect();

        let snap = CommandSnapshot::from_iters(states.into_iter(), std::iter::empty());

        // Send-safe state should be in snapshot
        assert!(
            snap.has_state::<SendSafeState>(),
            "Send-safe state should be in command snapshot"
        );
        assert_eq!(snap.state::<SendSafeState>().value, 42);

        // UI-affine state should NOT be in snapshot
        assert!(
            !snap.has_state::<UiAffineState>(),
            "UI-affine state should NOT be in command snapshot"
        );
        assert!(
            snap.try_state::<UiAffineState>().is_none(),
            "try_state for UI-affine should return None"
        );
    }
}
