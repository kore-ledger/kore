// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance module.
//!

use identity::identifier::DigestIdentifier;

use serde::{Deserialize, Serialize};

/// Governance struct.
///
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Governance {
    /// The governance's id.
    id: DigestIdentifier,
    /// The schema's id.
    schema_id: String,
    /// The governance's version.
    version: u64,
}
