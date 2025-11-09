use flume::{Receiver, Sender, unbounded};

use super::StateID;

use crate::{basic_states::BasicStates, state::State};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncStatus {
    #[default]
    Init,
    Pending,
    Dirty,
    Clean,
}

#[derive(Debug)]
pub struct StateCtx {
    send: Sender<BasicStates>,
    recv: Receiver<BasicStates>,

    // simple state tracking
    state_status: [SyncStatus; StateID::amount()],
    storage: Vec<BasicStates>,
}

impl StateCtx {
    pub fn new() -> Self {
        let (send, recv) = unbounded();
        let status = [SyncStatus::Init; StateID::amount()];

        Self {
            send,
            recv,
            state_status: status,
            storage: Vec::with_capacity(StateID::amount()),
        }
    }

    pub fn cached<T: State>(&self, _id: StateID) -> Option<&T> {
        unimplemented!()
    }

    pub fn mark_dirty(&mut self, id: StateID) {
        self.state_status[id as usize] = SyncStatus::Dirty;
    }

    pub fn mark_pending(&mut self, id: StateID) {
        self.state_status[id as usize] = SyncStatus::Pending;
    }

    pub fn mark_clean(&mut self, id: StateID) {
        self.state_status[id as usize] = SyncStatus::Clean;
    }

    pub fn clear(&mut self) {
        self.storage.clear();
    }
}
