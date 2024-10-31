// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Error module.
//!

use thiserror::Error;

use serde::{Deserialize, Serialize};

/// Error type.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    /// Node error.
    #[error("Node error: {0}")]
    Node(String),
    /// Database error.
    #[error("Database error: {0}")]
    Database(String),
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
    /// Request Handler
    #[error("Request handler error: {0}")]
    RequestHandler(String),
    /// Governance error.
    #[error("Governance error: {0}")]
    Governance(String),
    /// Subject error.
    #[error("Subject error: {0}")]
    Subject(String),
    /// Event error.
    #[error("Event error: {0}")]
    Event(String),
    /// SN error.
    #[error("SN error: {0}")]
    Sn(String),
    /// Validation error.
    #[error("Validation error: {0}")]
    Validation(String),
    /// Evaluation error.
    #[error("Evaluation error: {0}")]
    Evaluation(String),
    /// Runner error.
    #[error("Runner error: {0}")]
    Runner(String),
    /// Compiler error.
    #[error("Compiler error: {0}")]
    Compiler(String),
    /// Approval error.
    #[error("Approval error: {0}")]
    Approval(String),
    /// Approval error.
    #[error("InvalidQuorum error: {0}")]
    InvalidQuorum(String),
    /// NetworkHelper error.
    #[error("NetworkHelper error: {0}")]
    NetworkHelper(String),
    /// Network error.
    #[error("Network error: {0}")]
    Network(String),
    /// JSONSchema error.
    #[error("JSONSchema error: {0}")]
    JSONSChema(String),
}
