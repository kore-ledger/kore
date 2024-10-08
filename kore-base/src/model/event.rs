// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Event data model.
//!

use super::{
    network::TimeOutResponse,
    request::EventRequest,
    signature::{Signature, Signed},
    wrapper::ValueWrapper,
    HashId,
};

use crate::{
model::Namespace, subject::Subject, Error,
};

use identity::{
    identifier::{
        derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
    },
    keys::{KeyMaterial, KeyPair},
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A struct representing an event.
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
pub struct ProofEvent {
    /// The identifier of the subject of the event.
    pub subject_id: DigestIdentifier,
    /// The signed event request.
    pub event_request: Signed<EventRequest>,
    /// The sequence number of the event.
    pub sn: u64,
    /// The version of the governance contract.
    pub gov_version: u64,
    /// The patch to apply to the state.
    pub value: LedgerValue,
    /// The hash of the state after applying the patch.
    pub state_hash: DigestIdentifier,
    /// Whether the evaluation was successful and the result was validated against the schema.
    pub eval_success: Option<bool>,
    /// Whether approval is required for the event to be applied to the state.
    pub appr_required: bool,
    /// Whether the event has been approved.
    pub appr_success: Option<bool>,
    /// The hash of the previous event.
    pub hash_prev_proof_event: DigestIdentifier,
    /// The set of evaluators who have evaluated the event.
    pub evaluators: Option<HashSet<Signature>>,
    /// The set of approvers who have approved the event.
    pub approvers: Option<HashSet<ProtocolsResponse>>,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
    Hash,
    PartialOrd,
    Ord,
)]
pub enum ProtocolsResponse {
    Signature(Signature),
    TimeOut(TimeOutResponse),
}

impl HashId for ProofEvent {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator)
            .map_err(|_| Error::Subject("HashId for Event Fails".to_string()))
    }
}

impl HashId for Signed<ProofEvent> {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |_| Error::Subject("HashId for Signed Event Fails".to_string()),
        )
    }
}

/// A struct representing an event.
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
pub struct Event {
    /// The identifier of the subject of the event.
    pub subject_id: DigestIdentifier,
    /// The signed event request.
    pub event_request: Signed<EventRequest>,
    /// The sequence number of the event.
    pub sn: u64,
    /// The version of the governance contract.
    pub gov_version: u64,
    /// The patch to apply to the state.
    pub value: LedgerValue,
    /// The hash of the state after applying the patch.
    pub state_hash: DigestIdentifier,
    /// Whether the evaluation was successful and the result was validated against the schema.
    pub eval_success: Option<bool>,
    /// Whether approval is required for the event to be applied to the state.
    pub appr_required: bool,
    /// Whether the event has been approved.
    pub appr_success: Option<bool>,

    pub vali_success: bool,
    /// The hash of the previous event.
    pub hash_prev_event: DigestIdentifier,
    /// The set of evaluators who have evaluated the event.
    pub evaluators: Option<HashSet<Signature>>,
    /// The set of approvers who have approved the event.
    pub approvers: Option<HashSet<ProtocolsResponse>>,

    pub validators: HashSet<ProtocolsResponse>,
}

impl HashId for Event {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator)
            .map_err(|_| Error::Subject("HashId for Event Fails".to_string()))
    }
}

impl HashId for Signed<Event> {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |_| Error::Subject("HashId for Signed Event Fails".to_string()),
        )
    }
}

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
pub struct Ledger {
    /// The identifier of the subject of the event.
    pub subject_id: DigestIdentifier,
    /// The signed event request.
    pub event_request: Signed<EventRequest>,
    /// The sequence number of the event.
    pub sn: u64,
    /// The version of the governance contract.
    pub gov_version: u64,
    /// The patch to apply to the state.
    pub value: LedgerValue,
    /// The hash of the state after applying the patch.
    pub state_hash: DigestIdentifier,
    /// Whether the evaluation was successful and the result was validated against the schema.
    pub eval_success: Option<bool>,
    /// Whether approval is required for the event to be applied to the state.
    pub appr_required: bool,
    pub appr_success: Option<bool>,
    /// Whether the event has been approved.
    pub vali_success: bool,
    /// The hash of the previous event.
    pub hash_prev_event: DigestIdentifier,
}

impl From<Event> for Ledger {
    fn from(value: Event) -> Self {
        Ledger {
            subject_id: value.subject_id,
            event_request: value.event_request,
            sn: value.sn,
            gov_version: value.gov_version,
            value: value.value,
            state_hash: value.state_hash,
            eval_success: value.eval_success,
            appr_required: value.appr_required,
            appr_success: value.appr_success,
            vali_success: value.vali_success,
            hash_prev_event: value.hash_prev_event,
        }
    }
}

impl HashId for Ledger {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator)
            .map_err(|_| Error::Subject("HashId for Ledger Fails".to_string()))
    }
}

impl HashId for Signed<Ledger> {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |_| Error::Subject("HashId for Signed Ledger Fails".to_string()),
        )
    }
}

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
pub enum LedgerValue {
    Patch(ValueWrapper),
    Error(String),
}

// TODO REVISAR ESTO, sobre todo la parte final donde se crea el evento
impl ProofEvent {
    pub fn from_create_request(
        subject_keys: &KeyPair,
        request: &Signed<EventRequest>,
        governance_version: u64,
        init_state: &ValueWrapper,
        derivator: DigestDerivator,
    ) -> Result<Self, Error> {
        let EventRequest::Create(start_request) = &request.content else {
            return Err(Error::Event("Invalid Event Request".to_string()));
        };
        let public_key = KeyIdentifier::new(
            subject_keys.get_key_derivator(),
            &subject_keys.public_key_bytes(),
        );

        let subject_id = Subject::subject_id(
            start_request.namespace.clone(),
            &start_request.schema_id,
            public_key,
            start_request.governance_id.clone(),
            governance_version,
            derivator,
        )?;
        let state_hash =
            DigestIdentifier::from_serializable_borsh(init_state, derivator)
                .map_err(|_| {
                    Error::Digest("Error converting state to hash".to_owned())
                })?;

        Ok(ProofEvent {
            subject_id,
            event_request: request.clone(),
            sn: 0,
            gov_version: governance_version,
            value: LedgerValue::Patch(init_state.clone()),
            state_hash,
            eval_success: None,
            appr_required: false,
            hash_prev_proof_event: DigestIdentifier::default(),
            evaluators: None,
            approvers: None,
            appr_success: None,
        })
    }
}
