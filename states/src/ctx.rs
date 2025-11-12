use flume::{Receiver, Sender, unbounded};

use super::BasicStates;
use super::State;
use super::StateID;
use super::StateSyncStatus;

#[derive(Debug)]
pub struct StateCtx {
    send: Sender<BasicStates>,
    recv: Receiver<BasicStates>,

    // simple state tracking
    state_status: [StateSyncStatus; StateID::amount()],
    storage: Vec<BasicStates>,
}

impl StateCtx {
    pub fn new() -> Self {
        let (send, recv) = unbounded();
        let status = [StateSyncStatus::Init; StateID::amount()];

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

    pub fn update(&mut self, id: StateID, value: BasicStates) {
        unimplemented!()
    }

    pub fn mark_dirty(&mut self, id: StateID) {
        self.state_status[id as usize] = StateSyncStatus::Dirty;
    }

    pub fn mark_pending(&mut self, id: StateID) {
        self.state_status[id as usize] = StateSyncStatus::Pending;
    }

    pub fn mark_clean(&mut self, id: StateID) {
        self.state_status[id as usize] = StateSyncStatus::Clean;
    }

    pub fn clear(&mut self) {
        self.storage.clear();
    }
}
