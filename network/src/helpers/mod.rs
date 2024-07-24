use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod service;
pub mod intermediary;

/// Command enumeration for the Helper service.
#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    /// Send a message to the given peer.
    SendMessage {
        /// The peer to send the message to.
        peer: Vec<u8>,
        /// The message to send.
        message: Vec<u8>,
    },
}

/// Helper errors.
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum Error {
    /// Worker error.
    #[error("Worker error: {0}")]
    Worker(String),
}
