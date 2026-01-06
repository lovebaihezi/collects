use std::{
    any::{Any, TypeId, type_name},
    fmt::Debug,
};

use flume::{Receiver, Sender};

use crate::{Compute, StateRuntime};

/// The `State` trait represents a fundamental unit of state in the application.
///
/// It provides basic identity and initialization logic for state objects.
pub trait State: Any + Debug {
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
        Err(_) => {
            panic!(
                "Failed to assign state: type mismatch, expected {:?}, but any unable to downcast to it",
                type_name::<T>(),
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
}

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
    /// Set a Compute value via the updater channel.
    ///
    /// This is used by Commands to update Compute values after async operations complete.
    pub fn set<T: Compute + Send + 'static>(&self, compute: T) {
        let id = TypeId::of::<T>();
        let boxed: Box<dyn Any + Send> = Box::new(compute);
        self.send.send(UpdateMessage::Compute(id, boxed)).unwrap();
    }

    /// Set a State value via the updater channel.
    ///
    /// This is used by Commands to update State values after async operations complete.
    /// For synchronous state mutations in UI code, prefer using `StateCtx::state_mut()` directly.
    pub fn set_state<T: State + Send + 'static>(&self, state: T) {
        let id = TypeId::of::<T>();
        let boxed: Box<dyn Any + Send> = Box::new(state);
        self.send.send(UpdateMessage::State(id, boxed)).unwrap();
    }
}

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

unsafe impl Send for Reader {}
