// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Event data model.
//!

use super::{
    request::{
        EOLRequest, EventRequest, FactRequest, StartRequest, TransferRequest,
    },
    signature::{Signature, Signed},
    wrapper::ValueWrapper,
    HashId,
};

use crate::{governance::init::init_state, model::Namespace, subject::Subject, Error};

use identity::{
    identifier::{
        derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
    },
    keys::{KeyMaterial, KeyPair},
};

use borsh::{BorshDeserialize, BorshSerialize};
use json_patch::diff;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
    pub patch: ValueWrapper,
    /// The hash of the state after applying the patch.
    pub state_hash: DigestIdentifier,
    /// Whether the evaluation was successful and the result was validated against the schema.
    pub eval_success: bool,
    /// Whether approval is required for the event to be applied to the state.
    pub appr_required: bool,
    /// Whether the event has been approved.
    pub approved: bool,
    /// The hash of the previous event.
    pub hash_prev_event: DigestIdentifier,
    /// The set of evaluators who have evaluated the event.
    pub evaluators: HashSet<Signature>,
    /// The set of approvers who have approved the event.
    pub approvers: HashSet<Signature>,
}

impl Event {
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
            Namespace::from(start_request.namespace.as_str()),
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

        Ok(Event {
            subject_id,
            event_request: request.clone(),
            sn: 0,
            gov_version: governance_version,
            patch: init_state.clone(),
            state_hash,
            eval_success: true,
            appr_required: false,
            approved: true,
            hash_prev_event: DigestIdentifier::default(),
            evaluators: HashSet::new(),
            approvers: HashSet::new(),
        })
    }
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
