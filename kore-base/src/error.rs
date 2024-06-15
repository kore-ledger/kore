// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Error module.
//!

use thiserror::Error;

/// Error type.
#[derive(Error, Debug, Clone)]
pub enum Error {
    /// An error occurred.
    #[error("An error occurred.")]
    Generic,
    /// Store error.
    #[error("Store error: {0}")]
    Store(String),
    /// Digest
    #[error("Digest error: {0}")]
    Digest(String),
    /// Signature
    #[error("Signature error: {0}")]
    Signature(String),
    #[error("Request event error: {0}")]
    RequestEvent(String),
}
