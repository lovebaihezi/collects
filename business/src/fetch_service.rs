use std::fmt::Debug;

use ehttp::{Request, Response, Result};

pub trait FetchService: Send + Sync + Debug {
    fn fetch(&self, request: Request, on_done: Box<dyn FnOnce(Result<Response>) + Send + 'static>);
}

#[derive(Debug, Default)]
pub struct EhttpFetcher;

impl FetchService for EhttpFetcher {
    fn fetch(&self, request: Request, on_done: Box<dyn FnOnce(Result<Response>) + Send + 'static>) {
        ehttp::fetch(request, on_done)
    }
}

#[cfg(feature = "test-utils")]
#[derive(Debug, Default)]
pub struct MockFetcher {
    pub response: Option<Result<Response>>,
}

#[cfg(feature = "test-utils")]
impl FetchService for MockFetcher {
    fn fetch(
        &self,
        _request: Request,
        on_done: Box<dyn FnOnce(Result<Response>) + Send + 'static>,
    ) {
        if let Some(response) = &self.response {
            on_done(response.clone());
        } else {
            on_done(Err("MockFetcher: no response set".to_string()));
        }
    }
}
