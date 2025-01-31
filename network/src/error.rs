// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Network errors.
//!

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Network errors.
#[derive(Debug, Error, Serialize, Deserialize, Clone)]
pub enum Error {
    /// Worker error.
    #[error("Worker error: {0}")]
    Worker(String),
    /// Network error.
    #[error("Network error: {0}")]
    Network(String),
    /// Transport error.
    #[error("Transport error: {0}")]
    Transport(String),
    /// DNS error
    #[error("DNS error: {0}")]
    Dns(String),
    /// Behaviour error.
    #[error("Behaviour error: {0}")]
    Behaviour(String),
    /// Address error.
    #[error("Address error: {0}")]
    Address(String),
    /// Command error.
    #[error("Command error: {0}")]
    Command(String),
}
