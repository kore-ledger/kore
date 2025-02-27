// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    Error,
    model::{HashId, ValueWrapper, request::EventRequest, signature::Signed},
};
use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use tracing::error;

const TARGET_REQUEST: &str = "Kore-Approval-Request";

/// A struct representing an approval request.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
)]
pub struct ApprovalReq {
    /// The signed event request.
    pub event_request: Signed<EventRequest>,
    /// The sequence number of the event.
    pub sn: u64,
    /// The version of the governance contract.
    pub gov_version: u64,
    /// The patch to apply to the state.
    pub patch: ValueWrapper,
    /// The hash of the state after applying the patch.
    pub state_hash: DigestIdentifier,
    /// The hash of the previous event.
    pub hash_prev_event: DigestIdentifier,
    /// The hash of the previous event.
    pub subject_id: DigestIdentifier,
}

impl HashId for ApprovalReq {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_REQUEST, "HashId for ApprovalReq fails: {}", e);
                Error::HashID(format!("HashId for ApprovalReq fails: {}", e))
            },
        )
    }
}
