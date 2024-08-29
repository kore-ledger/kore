// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{model::HashId, Error, Signed};
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use super::request::ApprovalRequest;

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
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |_| {
                Error::Approval("HashId for ApprovalResponse Fails".to_string())
            },
        )
    }
}


/// An enum representing the state of an approval entity.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum ApprovalState {
    /// The approval entity is pending a response.
    Pending,
    /// Request for approval which is in responded status and accepted
    RespondedAccepted,
    /// Request for approval which is in responded status and rejected
    RespondedRejected,
    /// The approval entity is obsolete.
    Obsolete,
}

/// A struct representing an approval entity.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct ApprovalEntity {
    /// The identifier of the approval entity.
    pub id: DigestIdentifier,
    /// The signed approval request.
    pub request: Signed<ApprovalRequest>,
    /// The signed approval response, if one has been received.
    pub response: Option<Signed<ApprovalResponse>>,
    /// The state of the approval entity.
    pub state: ApprovalState,
    /// The sender of the approval request.
    pub sender: KeyIdentifier,
}