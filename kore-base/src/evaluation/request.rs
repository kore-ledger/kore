// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    model::{request::EventRequest, signature::Signed, HashId, ValueWrapper},
    Error,
};
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// A struct representing an evaluation request.
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
pub struct EvaluationReq {
    /// The signed event request.
    pub event_request: Signed<EventRequest>,
    /// The context in which the evaluation is being performed.
    pub context: SubjectContext,
    /// The sequence number of the event.
    pub sn: u64,
    /// The version of the governance contract.
    pub gov_version: u64,
}

/// A struct representing the context in which the evaluation is being performed.
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
pub struct SubjectContext {
    pub governance_id: DigestIdentifier,
    pub schema_id: String,
    pub is_owner: bool,
    pub state: ValueWrapper,
    pub namespace: String,
}

impl HashId for EvaluationReq {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |_| {
                Error::Evaluation(
                    "HashId for EvaluationRequest Fails".to_string(),
                )
            },
        )
    }
}
