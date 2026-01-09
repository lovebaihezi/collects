//! Task management types for structured concurrency.
//!
//! This module provides `TaskId` and `TaskHandle` for managing async tasks with
//! cooperative cancellation via `CancellationToken` from `tokio_util`.
//!
//! # Overview
//!
//! - `TaskId`: A unique identifier for spawned tasks, combining `TypeId` and generation counter
//! - `TaskHandle`: Wraps a task with its `CancellationToken` for cooperative cancellation
//!
//! # Usage
//!
//! ```ignore
//! use collects_states::{TaskId, TaskHandle};
//! use tokio_util::sync::CancellationToken;
//!
//! let token = CancellationToken::new();
//! let task_id = TaskId::new(TypeId::of::<MyCompute>(), 1);
//! let handle = TaskHandle::new(task_id, token);
//!
//! // Later, cancel the task
//! handle.cancel();
//! ```

use std::any::TypeId;

use tokio_util::sync::CancellationToken;

/// Unique identifier for a spawned task.
///
/// Combines a `TypeId` (to identify the compute/command type) with a generation counter
/// (to distinguish multiple tasks of the same type). This enables:
/// - Tracking which compute type spawned the task
/// - Identifying stale results when multiple tasks of the same type are spawned
/// - Auto-canceling previous tasks when a new task is spawned for the same compute type
///
/// # Example
///
/// ```ignore
/// let task_id = TaskId::new(TypeId::of::<ApiStatus>(), 1);
/// let next_id = TaskId::new(TypeId::of::<ApiStatus>(), 2);
///
/// // Same compute type, different generation
/// assert_eq!(task_id.type_id(), next_id.type_id());
/// assert_ne!(task_id.generation(), next_id.generation());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId {
    type_id: TypeId,
    generation: u64,
}

impl TaskId {
    /// Creates a new `TaskId` with the given type and generation.
    pub fn new(type_id: TypeId, generation: u64) -> Self {
        Self { type_id, generation }
    }

    /// Returns the `TypeId` component of this task identifier.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the generation counter of this task identifier.
    ///
    /// Higher generation values indicate more recently spawned tasks.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Handle to a spawned async task with cooperative cancellation support.
///
/// `TaskHandle` wraps a `CancellationToken` from `tokio_util` along with a `TaskId`
/// to enable:
/// - Cooperative cancellation via `cancel()`
/// - Checking if cancellation was requested via `is_cancelled()`
/// - Identifying the task via its `TaskId`
///
/// # Cooperative Cancellation Pattern
///
/// Tasks should periodically check `token.is_cancelled()` or use `tokio::select!`
/// with `token.cancelled()` to respond to cancellation requests gracefully.
///
/// ```ignore
/// async fn my_compute(cancel: CancellationToken) -> Result<(), Error> {
///     tokio::select! {
///         _ = cancel.cancelled() => {
///             // Cancellation was requested
///             Err(Error::Cancelled)
///         }
///         result = do_async_work() => {
///             result
///         }
///     }
/// }
/// ```
///
/// # Example
///
/// ```ignore
/// let token = CancellationToken::new();
/// let task_id = TaskId::new(TypeId::of::<MyCompute>(), 1);
/// let handle = TaskHandle::new(task_id, token);
///
/// // Check if cancelled
/// assert!(!handle.is_cancelled());
///
/// // Request cancellation
/// handle.cancel();
/// assert!(handle.is_cancelled());
/// ```
#[derive(Debug, Clone)]
pub struct TaskHandle {
    id: TaskId,
    cancel_token: CancellationToken,
}

impl TaskHandle {
    /// Creates a new `TaskHandle` with the given ID and cancellation token.
    pub fn new(id: TaskId, cancel_token: CancellationToken) -> Self {
        Self { id, cancel_token }
    }

    /// Returns the `TaskId` of this task.
    pub fn id(&self) -> TaskId {
        self.id
    }

    /// Returns a clone of the cancellation token.
    ///
    /// Use this to pass the token to async work that needs to check for cancellation.
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    /// Requests cooperative cancellation of this task.
    ///
    /// This signals the task to stop at its next cancellation check point.
    /// It does not forcibly abort the task - the task must cooperatively
    /// check `is_cancelled()` or await `cancelled()`.
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// Returns `true` if cancellation has been requested for this task.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_new_and_accessors() {
        let type_id = TypeId::of::<String>();
        let task_id = TaskId::new(type_id, 42);

        assert_eq!(task_id.type_id(), type_id);
        assert_eq!(task_id.generation(), 42);
    }

    #[test]
    fn task_id_equality() {
        let type_id = TypeId::of::<String>();

        let id1 = TaskId::new(type_id, 1);
        let id2 = TaskId::new(type_id, 1);
        let id3 = TaskId::new(type_id, 2);
        let id4 = TaskId::new(TypeId::of::<i32>(), 1);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3); // Different generation
        assert_ne!(id1, id4); // Different type
    }

    #[test]
    fn task_id_copy() {
        let type_id = TypeId::of::<String>();
        let id1 = TaskId::new(type_id, 1);
        let id2 = id1; // Copy

        assert_eq!(id1, id2);
    }

    #[test]
    fn task_handle_new_and_accessors() {
        let token = CancellationToken::new();
        let task_id = TaskId::new(TypeId::of::<String>(), 1);
        let handle = TaskHandle::new(task_id, token);

        assert_eq!(handle.id(), task_id);
        assert!(!handle.is_cancelled());
    }

    #[test]
    fn task_handle_cancel() {
        let token = CancellationToken::new();
        let task_id = TaskId::new(TypeId::of::<String>(), 1);
        let handle = TaskHandle::new(task_id, token);

        assert!(!handle.is_cancelled());
        handle.cancel();
        assert!(handle.is_cancelled());
    }

    #[test]
    fn task_handle_clone() {
        let token = CancellationToken::new();
        let task_id = TaskId::new(TypeId::of::<String>(), 1);
        let handle1 = TaskHandle::new(task_id, token);
        let handle2 = handle1.clone();

        // Both handles share the same cancellation token
        assert!(!handle1.is_cancelled());
        assert!(!handle2.is_cancelled());

        handle1.cancel();

        // Cancelling one cancels both (shared token)
        assert!(handle1.is_cancelled());
        assert!(handle2.is_cancelled());
    }

    #[test]
    fn task_handle_cancellation_token() {
        let token = CancellationToken::new();
        let task_id = TaskId::new(TypeId::of::<String>(), 1);
        let handle = TaskHandle::new(task_id, token);

        // Get a clone of the token
        let cloned_token = handle.cancellation_token();

        assert!(!cloned_token.is_cancelled());
        handle.cancel();
        assert!(cloned_token.is_cancelled());
    }

    #[test]
    fn task_id_hash() {
        use std::collections::HashSet;

        let type_id = TypeId::of::<String>();
        let id1 = TaskId::new(type_id, 1);
        let id2 = TaskId::new(type_id, 2);
        let id3 = TaskId::new(type_id, 1); // Same as id1

        let mut set = HashSet::new();
        set.insert(id1);
        set.insert(id2);
        set.insert(id3); // Should not increase size (duplicate of id1)

        assert_eq!(set.len(), 2);
        assert!(set.contains(&id1));
        assert!(set.contains(&id2));
    }
}
