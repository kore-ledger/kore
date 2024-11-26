// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Error module.
//!

use thiserror::Error;

use serde::{Deserialize, Serialize};

/// Error type.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    /// NetworkHelper error.
    #[error("NetworkHelper error: {0}")]
    NetworkHelper(String),
    /// Network error.
    #[error("Network error: {0}")]
    Network(String),
    /// Ext_db schema error.
    #[error("Ext DB error: {0}")]
    ExtDB(String),
    /// JSON schema error.
    #[error("JSON schema error: {0}")]
    JSONSChema(String),
    /// System error.
    #[error("System error: {0}")]
    System(String),
    /// Protocols error.
    #[error("Protocols error: {0}")]
    Protocols(String),
    /// Runner error.
    #[error("Runner error: {0}")]
    Runner(String),
    /// Compiler error.
    #[error("Compiler error: {0}")]
    Compiler(String),
    /// SN error.
    #[error("SN error: Incorrect sn ledger")]
    Sn,
    /// Register error.
    #[error("Register error: {0}")]
    Register(String),
    /// Auth error.
    #[error("Auth error: {0}")]
    Auth(String),
    /// Query error.
    #[error("Query error: {0}")]
    Query(String),
    /// Init error.
    #[error("Init error: {0}")]
    Init(String),
    /// RelationShip error.
    #[error("NoRelationShipde error: {0}")]
    RelationShip(String),
    /// Node error.
    #[error("Node error: {0}")]
    Node(String),
    /// Database error.
    #[error("Database error: {0}")]
    Database(String),
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
    /// Validation error.
    #[error("Validation error: {0}")]
    Validation(String),
    /// Evaluation error.
    #[error("Evaluation error: {0}")]
    Evaluation(String),

    /// Approval error.
    #[error("Approval error: {0}")]
    Approval(String),
    /// Approval error.
    #[error("InvalidQuorum error: {0}")]
    InvalidQuorum(String),
}
