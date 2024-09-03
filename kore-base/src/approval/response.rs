// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::default;

use crate::{model::{HashId, TimeStamp}, Error, Signature, Signed};
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use super::request::ApprovalRequest;
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
)]
pub struct ApprovalError {
    pub who: KeyIdentifier,
    pub error: String,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
)]
pub struct ApprovalTimeOut {
    pub who: KeyIdentifier,
    pub re_trys: u32,
    pub timestamp: TimeStamp,
}
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
pub enum ApprovalResponse {
    Signature(Signature,bool),
    TimeOut(ApprovalTimeOut),
    Error(ApprovalError)
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
#[derive(Default, Debug, Clone, Serialize, Deserialize, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum ApprovalState {
    /// The approval entity is pending a response.
    #[default]
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
    // pub response: Option<Signed<ApprovalResponse>>,
    /// The state of the approval entity.
    pub state: ApprovalState,
    // The sender of the approval request.
    // pub sender: KeyIdentifier,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize,PartialEq)]
pub enum VotationType {
    #[default]
    Normal,
    AlwaysAccept,
}

impl From<u8> for VotationType {
    fn from(passvotation: u8) -> Self {
        match passvotation {
            1 => Self::AlwaysAccept,
            _ => Self::Normal,
        }
    }
}