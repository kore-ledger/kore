//

use thiserror::Error;

/// Errors related to the Kore Client.
#[derive(Error, Debug)]
pub enum Error {
    /// Initialization error.
    #[error("Initialization error: {0}")]
    Init(String),
    /// Network related error.
    #[error("Network error: {0}")]
    Network(String),
}