// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Validation module.
//!


use crate::{
    governance::Governance, model::{
        event::Event as KoreEvent, request::EventRequest, signature::{Signature, Signed},
        HashId, Namespace,
    }, subject::SubjectState, Error, DIGEST_DERIVATOR
};
use actor::{Actor, ActorContext, Message, Event, Response, Handler, Error as ActorError};

use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::{
    derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
};
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tracing::{debug, error};

use std::collections::HashSet;

pub struct Validator {
    subject: SubjectState,
    governance: Governance,
    event: Signed<KoreEvent>,
}

impl Message for ValidationCommand {}

impl Event for ValidationEvent {}

impl Response for ValidationResponse {}

impl Actor for Validator {
    type Message = ValidationCommand;
    type Event = ValidationEvent;
    type Response = ValidationResponse;
}

#[async_trait]
impl Handler<Validator> for Validator {

    async fn handle_message(
        &mut self,
        message: ValidationCommand,
        ctx: &mut ActorContext<Validator>,
    ) -> Result<ValidationResponse, ActorError> {
        match message {
            ValidationCommand::Create(info) => {
                Ok(ValidationResponse::None)
            }
            ValidationCommand::Validate(event) => {
                // Validate the event.
                // If the event is valid, return the validation signature.
                // If the event is invalid, return an error.
                Ok(ValidationResponse::None)
            }
        }
    }

}


/// A struct representing a validation proof.
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
pub struct ValidationProof {
    /// The identifier of the subject being validated.
    pub subject_id: DigestIdentifier,
    /// The identifier of the schema used to validate the subject.
    pub schema_id: String,
    /// The namespace of the subject being validated.
    pub namespace: Namespace,
    /// The name of the subject being validated.
    pub name: String,
    /// The identifier of the public key of the subject being validated.
    pub subject_public_key: KeyIdentifier,
    /// The identifier of the governance contract associated with the subject being validated.
    pub governance_id: DigestIdentifier,
    /// The version of the governance contract that created the subject being validated.
    pub genesis_governance_version: u64,
    /// The sequence number of the subject being validated.
    pub sn: u64,
    /// The identifier of the previous event in the validation chain.
    pub prev_event_hash: DigestIdentifier,
    /// The identifier of the current event in the validation chain.
    pub event_hash: DigestIdentifier,
    /// The version of the governance contract used to validate the subject.
    pub governance_version: u64,
}

impl Default for ValidationProof {
    fn default() -> Self {
        Self {
            governance_id: DigestIdentifier::default(),
            governance_version: 0,
            subject_id: DigestIdentifier::default(),
            sn: 0,
            schema_id: "subjet_id".to_string(),
            namespace: Namespace::default(),
            prev_event_hash: DigestIdentifier::default(),
            event_hash: DigestIdentifier::default(),
            subject_public_key: KeyIdentifier::default(),
            genesis_governance_version: 0,
            name: "name".to_string(),
        }
    }
}

impl HashId for ValidationProof {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |_| {
                Error::Validation(
                    "Hashing error in ValidationProof".to_string(),
                )
            },
        )
    }
}

impl ValidationProof {

    /// Create a new validation proof from a validation command.
    pub fn from_info(info: ValidationInfo) -> Result<Self, Error> {
        let request = &info.event.content.event_request.content;

        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            derivator.clone()
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };
        let event_hash = info.event.hash_id(derivator)
            .map_err(|_| Error::Validation("Error hashing event".to_string()))?;
        match request {
            EventRequest::Create(start_request) => {
                Ok(Self {
                    governance_id: start_request.governance_id.clone(),
                    governance_version: info.gov_version,
                    subject_id: info.subject.subject_id.clone(),
                    sn: 0,
                    schema_id: start_request.schema_id.clone(),
                    namespace: Namespace::from(start_request.namespace.as_str()),
                    prev_event_hash: DigestIdentifier::default(),
                    event_hash,
                    subject_public_key: start_request.public_key.clone(),
                    genesis_governance_version: info.gov_version,
                    name: start_request.name.clone(),
                })
            }
            EventRequest::EOL(_) | EventRequest::Fact(_) => {
                Ok(Self {
                    governance_id: info.subject.governance_id.clone(),
                    governance_version: info.gov_version,
                    subject_id: info.subject.subject_id.clone(),
                    sn: info.event.content.sn,
                    schema_id: info.subject.schema_id.clone(),
                    namespace: info.subject.namespace.clone(),
                    prev_event_hash: info.event.content.hash_prev_event,
                    event_hash,
                    subject_public_key: info.subject.subject_key.clone(),
                    genesis_governance_version: info.subject.genesis_gov_version,
                    name: info.subject.name.clone(),
                })
            },
            EventRequest::Transfer(transfer) => {
                Ok(Self {
                    governance_id: info.subject.governance_id.clone(),
                    governance_version: info.gov_version,
                    subject_id: info.subject.subject_id.clone(),
                    sn: info.event.content.sn,
                    schema_id: info.subject.schema_id.clone(),
                    namespace: info.subject.namespace.clone(),
                    prev_event_hash: info.event.content.hash_prev_event,
                    event_hash,
                    subject_public_key: transfer.public_key.clone(),
                    genesis_governance_version: info.subject.genesis_gov_version,
                    name: info.subject.name.clone(),
                })
            },
        }
    }
}

#[derive(Clone)]
pub struct ValidationInfo {
    pub subject: SubjectState,
    pub event: Signed<KoreEvent>,
    pub gov_version: u64,
}
#[derive(Clone)]
pub enum ValidationCommand {
    Create(ValidationInfo),
    Validate(ValidationEvent),
}

/// A struct representing a validation event.
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ValidationEvent {
    pub proof: ValidationProof,
    pub subject_signature: Signature,
    pub previous_proof: Option<ValidationProof>,
    pub prev_event_validation_signatures: HashSet<Signature>,
}

/// A struct representing a validation response.
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
pub enum ValidationResponse {
    Signature{
        validation_signature: Signature,
        gov_version_validation: u64,
    },
    None,
}
