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
            schema_id: "subject_id".to_string(),
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
        debug!("Creating validation proof from info");
        let request = &info.event.content.event_request.content;

        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            derivator.clone()
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };
        let event_hash = info.event.hash_id(derivator).map_err(|_| {
            Error::Validation("Error hashing event".to_string())
        })?;
        match request {
            EventRequest::Create(start_request) => Ok(Self {
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
            }),
            EventRequest::EOL(_) | EventRequest::Fact(_) => Ok(Self {
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
            }),
            EventRequest::Transfer(transfer) => Ok(Self {
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
            }),
        }
    }
}
