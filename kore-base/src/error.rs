// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Error module.
//!

use thiserror::Error;

use serde::{Deserialize, Serialize};

/// Error type.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    /// An error occurred.
    #[error("An error occurred.")]
    Generic,
    /// Actor error.
    #[error("Actor error: {0}")]
    Actor(String),
    /// Store error.
    #[error("Store error: {0}")]
    Store(String),
    /// Digest
    #[error("Digest error: {0}")]
    Digest(String),
    /// Signature
    #[error("Signature error: {0}")]
    Signature(String),
    /// Password
    #[error("Password error: {0}")]
    Password(String),
    /// Request event
    #[error("Request event error: {0}")]
    RequestEvent(String),
    /// Governance error.
    #[error("Governance error: {0}")]
    Governance(String),
}
