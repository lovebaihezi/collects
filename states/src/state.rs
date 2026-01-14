use std::{
    any::{Any, TypeId, type_name},
    fmt::Debug,
};

use flume::{Receiver, Sender};

use crate::{Compute, SnapshotClone, StateRuntime};

use crate::task::TaskId;

/// An updater wrapper that enforces "latest-only" semantics for async publishers.
///
/// This is intended to make out-of-order completion safe:
/// - Tasks capture their `TaskId` at spawn time.
/// - Before publishing, they check whether they are still the active task (same generation).
/// - If not current, the write is dropped (and debug-asserted).
///
/// Notes:
/// - This is intentionally lightweight: it only gates writes, it does not schedule/await.
/// - Stale writes are a logic error; in debug builds we `debug_assert!` to surface them early.
/// - The gate is keyed by an arbitrary `TypeId` (not necessarily `T` of the payload being set).
#[derive(Clone)]
pub struct LatestOnlyUpdater {
    updater: Updater,
    gate_type_id: TypeId,
    task_id: TaskId,
    current_check: std::sync::Arc<dyn Fn(TaskId) -> bool + Send + Sync>,
}

impl LatestOnlyUpdater {
    #[inline]
    fn is_current(&self) -> bool {
        // Defensive: ensure the check is applied to the correct gate type.
        if self.task_id.type_id() != self.gate_type_id {
            return false;
        }
        (self.current_check)(self.task_id)
    }

    /// Publish a compute update only if the task is still current for this updater's gate `TypeId`.
    pub fn set<T: Compute + Send + 'static>(&self, compute: T) {
        if !self.is_current() {
            #[cfg(debug_assertions)]
            debug_assert!(
                false,
                "stale async publish attempt (compute) for gate={:?}, task_id={:?}",
                self.gate_type_id, self.task_id
            );
            return;
        }
        self.updater.set(compute);
    }

    /// Publish a state update only if the task is still current for this updater's gate `TypeId`.
    pub fn set_state<T: State + Send + 'static>(&self, state: T) {
        if !self.is_current() {
            #[cfg(debug_assertions)]
            debug_assert!(
                false,
                "stale async publish attempt (state) for gate={:?}, task_id={:?}",
                self.gate_type_id, self.task_id
            );
            return;
        }
        self.updater.set_state(state);
    }

    /// Enqueue a command only if the task is still current for this updater's gate `TypeId`.
    pub fn enqueue_command<T: crate::Command + 'static>(&self) {
        if !self.is_current() {
            #[cfg(debug_assertions)]
            debug_assert!(
                false,
                "stale async publish attempt (enqueue_command) for gate={:?}, task_id={:?}",
                self.gate_type_id, self.task_id
            );
            return;
        }
        self.updater.enqueue_command::<T>();
    }

    /// Run an async block (typically "the task body") while providing this latest-only updater.
    ///
    /// This is a small ergonomic helper so call sites can do:
    /// `latest.run(async move { ... latest.set(...) ... }).await`
    pub async fn run<Fut: std::future::Future<Output = ()>>(self, fut: Fut) {
        fut.await;
    }
}

/// The `State` trait represents a fundamental unit of state in the application.
///
/// It provides basic identity and initialization logic for state objects.
pub trait State: Any + Debug + SnapshotClone {
    /// Returns a reference to self as `&dyn Any` for read-only downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Returns a mutable reference to self as `&mut dyn Any` for mutable downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Initializes the state.
    ///
    /// This method is called when the state is first added to the context.
    fn init(&mut self) {}

    /// Returns the name of the state type.
    ///
    /// Defaults to the type name.
    fn name(&self) -> &'static str {
        type_name::<Self>()
    }

    /// Returns the unique type ID of the state.
    fn id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    /// Assigns a new value to this state from a boxed `Any`.
    ///
    /// Used for updating the state's value via the `Updater` channel.
    /// States that can be updated via `Updater::set_state()` should implement this.
    /// States that cannot be sent across threads (e.g., containing `TextureHandle`)
    /// can leave the default implementation which panics.
    ///
    /// For UI-only state mutations, use `StateCtx::state_mut()` directly instead.
    fn assign_box(&mut self, _new_self: Box<dyn Any + Send>) {
        panic!(
            "State type {} does not support assign_box. \
            Use StateCtx::state_mut() for direct mutation instead of Updater::set_state().",
            type_name::<Self>()
        );
    }
}

/// Helper function to implement `assign_box` for State types.
///
/// Usage in State impl:
/// ```ignore
/// fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
///     state_assign_impl(self, new_self);
/// }
/// ```
pub fn state_assign_impl<T: State + 'static>(old: &mut T, new: Box<dyn Any + Send>) {
    match new.downcast::<T>() {
        Ok(value) => {
            log::debug!(
                "Assign New State {:?} to State {:?}",
                &value,
                type_name::<T>()
            );
            *old = *value;
        }
        Err(boxed_any) => {
            panic!(
                "Failed to assign state: type mismatch, expected {:?}, got {:?}",
                type_name::<T>(),
                (*boxed_any).type_id(),
            );
        }
    }
}

/// Message type for updates via Updater.
#[derive(Debug)]
pub enum UpdateMessage {
    /// Update a Compute type.
    Compute(TypeId, Box<dyn Any + Send>),
    /// Update a State type.
    State(TypeId, Box<dyn Any + Send>),
    /// Enqueue a command to be executed at end-of-frame.
    ///
    /// This allows Computes to request command execution without doing IO themselves.
    /// The command will be added to the command queue during `sync_computes()`.
    EnqueueCommand(TypeId),
}

#[derive(Debug, Clone)]
pub struct Updater {
    send: Sender<UpdateMessage>,
}

impl From<&StateRuntime> for Updater {
    fn from(run_time: &StateRuntime) -> Self {
        Self {
            send: run_time.sender(),
        }
    }
}

impl Updater {
    /// Create a latest-only updater bound to a specific task identity.
    ///
    /// Intended usage:
    /// - A caller creates a `TaskId` with an appropriate `type_id` "gate" (e.g. the task/command type).
    /// - The async work uses this wrapper for *all* updates.
    /// - If the task is no longer current for the associated gate, writes are dropped.
    ///
    /// Notes:
    /// - The gate is derived from `task_id.type_id()` rather than from a generic `T`.
    pub fn latest_only(
        &self,
        task_id: TaskId,
        current_check: std::sync::Arc<dyn Fn(TaskId) -> bool + Send + Sync>,
    ) -> LatestOnlyUpdater {
        LatestOnlyUpdater {
            updater: self.clone(),
            gate_type_id: task_id.type_id(),
            task_id,
            current_check,
        }
    }

    /// Set a Compute value via the updater channel.
    ///
    /// This is used by Commands to update Compute values after async operations complete.
    pub fn set<T: Compute + Send + 'static>(&self, compute: T) {
        let id = TypeId::of::<T>();
        let boxed: Box<dyn Any + Send> = Box::new(compute);
        self.send
            .send(UpdateMessage::Compute(id, boxed))
            .expect("Updater channel closed unexpectedly");
    }

    /// Set a State value via the updater channel.
    ///
    /// This is used by Commands to update State values after async operations complete.
    /// For synchronous state mutations in UI code, prefer using `StateCtx::state_mut()` directly.
    pub fn set_state<T: State + Send + 'static>(&self, state: T) {
        let id = TypeId::of::<T>();
        let boxed: Box<dyn Any + Send> = Box::new(state);
        self.send
            .send(UpdateMessage::State(id, boxed))
            .expect("Updater channel closed unexpectedly");
    }

    /// Enqueue a command to be executed at end-of-frame.
    ///
    /// This allows Computes to request command execution without performing IO themselves.
    /// The command will be added to the command queue during the next `sync_computes()` call
    /// and executed during `flush_commands()`.
    ///
    /// This is the recommended way for Computes to trigger async operations:
    /// - Compute checks conditions (e.g., time elapsed, retry logic)
    /// - If action needed, calls `updater.enqueue_command::<FetchCommand>()`
    /// - Command performs the actual network IO
    ///
    /// # Example
    ///
    /// ```ignore
    /// impl Compute for ApiStatus {
    ///     fn compute(&self, deps: Dep, updater: Updater) {
    ///         let now = deps.get_state_ref::<Time>().as_ref().to_utc();
    ///         if self.should_fetch(now) {
    ///             updater.enqueue_command::<FetchApiStatusCommand>();
    ///         }
    ///     }
    /// }
    /// ```
    pub fn enqueue_command<T: crate::Command + 'static>(&self) {
        let id = TypeId::of::<T>();
        self.send
            .send(UpdateMessage::EnqueueCommand(id))
            .expect("Updater channel closed unexpectedly");
    }
}

// SAFETY: Updater only contains a Sender<UpdateMessage> which is Send.
// The channel is thread-safe and messages are self-contained values.
unsafe impl Send for Updater {}

pub struct Reader {
    recv: Receiver<UpdateMessage>,
}

impl Reader {
    /// Try to receive a message from the channel.
    pub fn try_recv(&self) -> Option<UpdateMessage> {
        self.recv.try_recv().ok()
    }

    /// Get the number of pending messages.
    pub fn len(&self) -> usize {
        self.recv.len()
    }

    /// Check if there are no pending messages.
    pub fn is_empty(&self) -> bool {
        self.recv.is_empty()
    }
}

impl From<&StateRuntime> for Reader {
    fn from(run_time: &StateRuntime) -> Self {
        Self {
            recv: run_time.receiver(),
        }
    }
}

// SAFETY: Reader only contains a Receiver<UpdateMessage> which is Send.
// The channel is thread-safe and messages are self-contained values.
unsafe impl Send for Reader {}
