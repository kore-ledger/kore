// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    Error, 
    model::HashId,
};
use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// A struct representing an approval response.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
    PartialEq,
    Hash,
)]
pub struct ApprovalResponse {
    /// The hash of the approval request being responded to.
    pub appr_req_hash: DigestIdentifier,
    /// Whether the approval request was approved or not.
    pub approved: bool,
}

impl HashId for ApprovalResponse {
    fn hash_id(&self, derivator: DigestDerivator) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(|_| {
            Error::Approval("HashId for ApprovalResponse Fails".to_string())
        })
    }
}
