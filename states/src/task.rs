//! Task management types for structured concurrency.
//!
//! This module provides `TaskId` and `TaskHandle` for managing async tasks with
//! cooperative cancellation via `CancellationToken` from `tokio_util`.
//!
//! # Overview
//!
//! - `TaskId`: A unique identifier for spawned tasks, combining `TypeId` and generation counter
//! - `TaskIdGenerator`: Thread-safe generator for `TaskId`s using atomics, cache-line aligned
//! - `TaskHandle`: Wraps a task with its `CancellationToken` for cooperative cancellation
//!
//! # Usage
//!
//! ```ignore
//! use collects_states::{TaskId, TaskIdGenerator, TaskHandle};
//! use tokio_util::sync::CancellationToken;
//!
//! let generator = TaskIdGenerator::new();
//! let task_id = generator.next::<MyCompute>();
//! let token = CancellationToken::new();
//! let handle = TaskHandle::new(task_id, token);
//!
//! // Later, cancel the task
//! handle.cancel();
//! ```

use std::any::TypeId;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio_util::sync::CancellationToken;

/// Cache line size for most modern CPUs (64 bytes).
const CACHE_LINE_SIZE: usize = 64;

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
        Self {
            type_id,
            generation,
        }
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

/// Thread-safe generator for `TaskId`s using atomic operations.
///
/// The generation counter is cache-line aligned to prevent false sharing
/// when multiple threads are generating task IDs concurrently.
///
/// # Memory Ordering
///
/// This generator uses `Ordering::Relaxed` for the atomic counter. This provides
/// unique IDs but does not guarantee that threads will observe generation values
/// in any particular order. This is acceptable for task IDs because:
/// - Each ID is unique (atomicity guarantees no duplicates)
/// - The ordering of task spawns across threads is inherently non-deterministic
/// - Task cancellation and completion don't depend on generation ordering
///
/// # Example
///
/// ```ignore
/// let generator = TaskIdGenerator::new();
/// let id1 = generator.next::<MyCompute>();
/// let id2 = generator.next::<MyCompute>();
///
/// assert_eq!(id1.type_id(), id2.type_id());
/// assert_ne!(id1.generation(), id2.generation()); // Different generations
/// ```
#[repr(align(64))] // Align to cache line size to prevent false sharing
#[derive(Debug)]
pub struct TaskIdGenerator {
    /// Atomic generation counter, padded to fill the cache line.
    generation: AtomicU64,
    /// Padding to ensure the struct fills a full cache line.
    _padding: [u8; CACHE_LINE_SIZE - std::mem::size_of::<AtomicU64>()],
}

// Compile-time assertion: AtomicU64 must fit within cache line
const _: () = assert!(
    std::mem::size_of::<AtomicU64>() <= CACHE_LINE_SIZE,
    "AtomicU64 size exceeds cache line size"
);

impl Default for TaskIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskIdGenerator {
    /// Creates a new `TaskIdGenerator` starting at generation 0.
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            _padding: [0; CACHE_LINE_SIZE - std::mem::size_of::<AtomicU64>()],
        }
    }

    /// Generates the next `TaskId` for the given type.
    ///
    /// Each call atomically increments the generation counter, ensuring
    /// unique IDs across concurrent calls. Uses `Ordering::Relaxed` since
    /// strict cross-thread ordering is not required for task ID uniqueness.
    pub fn next<T: 'static>(&self) -> TaskId {
        let generation = self.generation.fetch_add(1, Ordering::Relaxed);
        TaskId::new(TypeId::of::<T>(), generation)
    }

    /// Generates the next `TaskId` for a given `TypeId`.
    ///
    /// Use this when the type is determined at runtime.
    /// Uses `Ordering::Relaxed` since strict cross-thread ordering is not
    /// required for task ID uniqueness.
    pub fn next_for(&self, type_id: TypeId) -> TaskId {
        let generation = self.generation.fetch_add(1, Ordering::Relaxed);
        TaskId::new(type_id, generation)
    }

    /// Returns the current generation value without incrementing.
    pub fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
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

    #[test]
    fn task_id_generator_new() {
        let generator = TaskIdGenerator::new();
        assert_eq!(generator.current_generation(), 0);
    }

    #[test]
    fn task_id_generator_next() {
        let generator = TaskIdGenerator::new();

        let id1 = generator.next::<String>();
        let id2 = generator.next::<String>();
        let id3 = generator.next::<i32>();

        assert_eq!(id1.generation(), 0);
        assert_eq!(id2.generation(), 1);
        assert_eq!(id3.generation(), 2);

        // Same type for id1 and id2, different for id3
        assert_eq!(id1.type_id(), id2.type_id());
        assert_ne!(id1.type_id(), id3.type_id());
    }

    #[test]
    fn task_id_generator_next_for() {
        let generator = TaskIdGenerator::new();
        let type_id = TypeId::of::<String>();

        let id1 = generator.next_for(type_id);
        let id2 = generator.next_for(type_id);

        assert_eq!(id1.type_id(), type_id);
        assert_eq!(id2.type_id(), type_id);
        assert_eq!(id1.generation(), 0);
        assert_eq!(id2.generation(), 1);
    }

    #[test]
    fn task_id_generator_cache_line_aligned() {
        // Verify the generator is cache-line aligned (64 bytes)
        assert_eq!(std::mem::align_of::<TaskIdGenerator>(), 64);
        assert_eq!(std::mem::size_of::<TaskIdGenerator>(), 64);
    }

    #[test]
    fn task_id_generator_default() {
        let generator = TaskIdGenerator::default();
        assert_eq!(generator.current_generation(), 0);
    }
}
