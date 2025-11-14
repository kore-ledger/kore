use thiserror::Error;

use serde::{Deserialize, Serialize};

/// Error type.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    /// HashID error.
    #[error("HashID error: {0}")]
    HashID(String),   
    /// Signature error.
    #[error("Signature error: {0}")]
    Signature(String),
}