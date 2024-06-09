// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Error module.
//!

use thiserror::Error;

/// Error type.
#[derive(Error, Debug)]
pub enum Error {
    /// An error occurred.
    #[error("An error occurred.")]
    Generic,
    /// Digest
    #[error("Digest error: {0}")]
    Digest(String),
    /// Signature
    #[error("Signature error: {0}")]
    Signature(String),

}
