use crate::FetchService;
use collects_states::State;

#[derive(Debug, Default)]
pub struct FetchState<S: FetchService> {
    pub inner: S,
}

impl<S: FetchService> State for FetchState<S> {}
