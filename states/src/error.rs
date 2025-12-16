use std::any::TypeId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("State not found: {id:?}, context: {context}")]
    StateNotFound {
        id: TypeId,
        context: String,
    },
    #[error("Compute not found: {id:?}, context: {context}")]
    ComputeNotFound {
        id: TypeId,
        context: String,
    },
}

impl Error {
    pub fn state_not_found(id: TypeId, context: impl Into<String>) -> Self {
        Self::StateNotFound {
            id,
            context: context.into(),
        }
    }

    pub fn compute_not_found(id: TypeId, context: impl Into<String>) -> Self {
        Self::ComputeNotFound {
            id,
            context: context.into(),
        }
    }
}
