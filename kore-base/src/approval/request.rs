// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    model::{request::EventRequest, signature::Signed, HashId, ValueWrapper},
    Error,
};
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

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

impl ApprovalReq {
    pub fn to_string(&self) -> String {
        format!(
            "
            subject_id: {},
            event_request: {:#?},
            sn: {},
            gov_version: {},
            patch: {:#?},
            state_hash: {},
            hash_prev_event: {},
            ",
            self.subject_id,
            self.event_request,
            self.sn,
            self.gov_version,
            self.patch,
            self.state_hash,
            self.hash_prev_event
        )
    }
}

impl HashId for ApprovalReq {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |_| Error::Approval("HashId for ApprovalRequest Fails".to_string()),
        )
    }
}
