// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::ValidationInfo;

use crate::{
    error,
    model::{request::EventRequest, HashId, Namespace},
    Error, DIGEST_DERIVATOR,
};
use identity::identifier::{
    derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

const TARGET_PROOF: &str = "Kore-Validation-Proof";

#[allow(clippy::upper_case_acronyms)]
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
        matches!(
            (self, other),
            (EventProof::Create, EventProof::Create)
                | (EventProof::Fact, EventProof::Fact)
                | (EventProof::Confirm, EventProof::Confirm)
                | (EventProof::EOL, EventProof::EOL)
                | (EventProof::Transfer { .. }, EventProof::Transfer { .. })
        )
    }
}

impl From<EventRequest> for EventProof {
    fn from(value: EventRequest) -> Self {
        match value {
            EventRequest::Create(_create_request) => EventProof::Create,
            EventRequest::Fact(_fact_request) => EventProof::Fact,
            EventRequest::Transfer(transfer_request) => EventProof::Transfer {
                new_owner: transfer_request.new_owner,
            },
            EventRequest::Confirm(_confirm_request) => EventProof::Confirm,
            EventRequest::EOL(_eolrequest) => EventProof::EOL,
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
    pub owner: KeyIdentifier,
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
            genesis_governance_version: 0,
            event: EventProof::Create,
            owner: KeyIdentifier::default()
        }
    }
}

impl HashId for ValidationProof {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_PROOF, "HashId for ValidationProof fails: {}", e);
                Error::HashID(format!(
                    "HashId for ValidationProof fails: {}",
                    e
                ))
            },
        )
    }
}

impl ValidationProof {
    /// Create a new validation proof from a validation command.
    pub fn from_info(
        info: ValidationInfo,
        prev_event_hash: DigestIdentifier,
    ) -> Result<Self, Error> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!(TARGET_PROOF, "Error getting derivator");
            DigestDerivator::Blake3_256
        };
        let event_hash = info.event_proof.hash_id(derivator)?;

        let validation_proof: ValidationProof = Self {
            governance_id: info.metadata.governance_id.clone(),
            governance_version: info.event_proof.content.gov_version,
            subject_id: info.metadata.subject_id.clone(),
            sn: info.event_proof.content.sn,
            schema_id: info.metadata.schema_id.clone(),
            namespace: info.metadata.namespace.clone(),
            prev_event_hash,
            event_hash,
            genesis_governance_version: info.metadata.genesis_gov_version,
            event: info.event_proof.content.event_proof.clone(),
            owner: info.metadata.owner.clone()
        };

        Ok(validation_proof)
    }
}
