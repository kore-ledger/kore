// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    Error,
    model::{
        HashId, Namespace, ValueWrapper, request::EventRequest,
        signature::Signed,
    },
};
use identity::identifier::{
    DigestIdentifier, KeyIdentifier, derive::digest::DigestDerivator,
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use tracing::error;

const TARGET_REQUEST: &str = "Kore-Evaluation-Request";
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

    pub gov_state_init_state: ValueWrapper,

    pub state: ValueWrapper,

    pub new_owner: Option<KeyIdentifier>,
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
    pub subject_id: DigestIdentifier,
    pub governance_id: DigestIdentifier,
    pub schema_id: String,
    pub is_owner: bool,
    pub namespace: Namespace,
}

impl HashId for EvaluationReq {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_REQUEST, "HashId for ProofEvent fails: {}", e);
                Error::HashID(format!("HashId for ProofEvent fails: {}", e))
            },
        )
    }
}
