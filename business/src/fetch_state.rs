use crate::FetchService;
use collects_states::State;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FetchState {
    pub inner: Arc<Box<dyn FetchService>>,
}

impl State for FetchState {}
