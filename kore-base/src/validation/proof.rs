// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::ValidationInfo;

use crate::{
    model::{request::EventRequest, HashId, Namespace},
    Error, DIGEST_DERIVATOR,
};
use identity::identifier::{
    derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use tracing::{debug, error};

#[derive(
    Debug, Clone, Serialize, Deserialize, Eq, BorshSerialize, BorshDeserialize,
)]
pub enum EventProof {
    Create,
    Fact,
    Transfer { new_owner: KeyIdentifier },
    Confirm,
    EOL,
}

// Implementación personalizada de PartialEq
impl PartialEq for EventProof {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (EventProof::Create, EventProof::Create)
            | (EventProof::Fact, EventProof::Fact)
            | (EventProof::Confirm, EventProof::Confirm)
            | (EventProof::EOL, EventProof::EOL)
            | (EventProof::Transfer { .. }, EventProof::Transfer { .. }) => {
                true
            }
            _ => false,
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
    pub event: EventProof,
}

impl Default for ValidationProof {
    fn default() -> Self {
        Self {
            governance_id: DigestIdentifier::default(),
            governance_version: 0,
            subject_id: DigestIdentifier::default(),
            sn: 0,
            schema_id: "subject_id".to_string(),
            namespace: Namespace::default(),
            prev_event_hash: DigestIdentifier::default(),
            event_hash: DigestIdentifier::default(),
            subject_public_key: KeyIdentifier::default(),
            genesis_governance_version: 0,
            name: "name".to_string(),
            event: EventProof::Create,
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
        debug!("Creating validation proof from info");
        let request = &info.event.content.event_request.content;

        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };
        let event_hash = info.event.hash_id(derivator).map_err(|_| {
            Error::Validation("Error hashing event".to_string())
        })?;

        let mut validation_proof: ValidationProof = Self {
            governance_id: info.subject.governance_id.clone(),
            governance_version: info.gov_version,
            subject_id: info.subject.subject_id.clone(),
            sn: info.event.content.sn,
            schema_id: info.subject.schema_id.clone(),
            namespace: info.subject.namespace.clone(),
            prev_event_hash: DigestIdentifier::default(),
            event_hash,
            subject_public_key: info.subject.subject_key.clone(),
            genesis_governance_version: info.gov_version,
            name: info.subject.name.clone(),
            event: EventProof::Create,
        };

        match request {
            EventRequest::Create(_start_request) => Ok(validation_proof),
            EventRequest::Fact(_fact_request) => {
                validation_proof.event = EventProof::Fact;
                Ok(validation_proof)
            }
            EventRequest::Transfer(transfer_request) => {
                validation_proof.event = EventProof::Transfer {
                    new_owner: transfer_request.new_owner.clone(),
                };
                Ok(validation_proof)
            }
            EventRequest::Confirm(_confirm_request) => {
                validation_proof.event = EventProof::Confirm;
                Ok(validation_proof)
            }
            EventRequest::EOL(_eol_request) => {
                validation_proof.event = EventProof::EOL;
                Ok(validation_proof)
            }
        }
    }
}
