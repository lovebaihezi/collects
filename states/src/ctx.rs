use std::collections::BTreeMap;

use flume::{Receiver, Sender, unbounded};

use crate::state::State;

pub type ID = usize;

pub struct SyncState {
    pub dirty: bool,
    pub data: Option<Box<dyn State>>,
}

pub struct StateCtx {
    send: Sender<Box<dyn State>>,
    recv: Receiver<Box<dyn State>>,

    storage: BTreeMap<ID, SyncState>,
}

impl StateCtx {
    pub fn new() -> Self {
        let (send, recv) = unbounded();

        Self {
            send,
            recv,
            storage: BTreeMap::new(),
        }
    }

    pub fn cached<T: State>(&self, id: ID) -> Option<&T> {
        let state = self.storage.get(id);
        state.and_then(|ss| ss.cast::<T>())
    }

    pub fn mark_dirty(&self, _id: ID) {
        unimplemented!()
    }

    pub fn mark_pending(&self, _id: ID) {
        unimplemented!()
    }

    pub fn mark_clean(&self, _id: ID) {
        unimplemented!()
    }

    pub fn clear(&self) {
        unimplemented!()
    }
}
