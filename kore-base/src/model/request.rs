// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Request data model.
//!

use super::{
    signature::{Signature, Signed},
    wrapper::ValueWrapper,
    HashId,
};

use crate::Error;

use identity::identifier::{
    derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// An enum representing a Kore Ledger event request.
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
pub enum EventRequest {
    /// A request to create a new subject.
    Create(StartRequest),
    /// A request to add a fact to a subject.
    Fact(FactRequest),
    /// A request to transfer ownership of a subject.
    Transfer(TransferRequest),
    /// A request to mark a subject as end-of-life.
    EOL(EOLRequest),
}

/// A struct representing a request to create a new subject.
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
pub struct StartRequest {
    /// The identifier of the governance contract.
    pub governance_id: DigestIdentifier,
    /// The identifier of the schema used to validate the event.
    pub schema_id: String,
    /// The namespace of the subject.
    pub namespace: String,
    /// The name of the subject.
    pub name: String,
    /// The identifier of the public key of the subject owner.
    pub public_key: KeyIdentifier,
}

/// A struct representing a request to add a fact to a subject.
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
pub struct FactRequest {
    /// The identifier of the subject to which the fact will be added.
    pub subject_id: DigestIdentifier,
    /// The payload of the fact to be added.
    pub payload: ValueWrapper,
}

/// A struct representing a request to transfer ownership of a subject.
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
pub struct TransferRequest {
    /// The identifier of the subject to transfer ownership of.
    pub subject_id: DigestIdentifier,
    /// The identifier of the public key of the new owner.
    pub new_owner: KeyIdentifier,
}

/// A struct representing a request to mark a subject as end-of-life.
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
pub struct EOLRequest {
    /// The identifier of the subject to mark as end-of-life.
    pub subject_id: DigestIdentifier,
}

impl EventRequest {
    pub fn requires_eval_appr(&self) -> bool {
        match self {
            EventRequest::Fact(_) => true,
            EventRequest::Create(_)
            | EventRequest::Transfer(_)
            | EventRequest::EOL(_) => false,
        }
    }
}

impl HashId for EventRequest {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |_| Error::Signature("HashId for EventRequest Fails".to_string()),
        )
    }
}

/// Indicates the current status of an event request.
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
pub enum RequestState {
    Finished,
    Error,
    Processing,
}

/// A struct representing a Kore Ledger request.
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub struct KoreRequest {
    /// The identifier of the request.
    pub id: DigestIdentifier,
    /// The identifier of the subject associated with the request, if any.
    pub subject_id: Option<DigestIdentifier>,
    /// The sequence number of the request, if any.
    pub sn: Option<u64>,
    /// The event request associated with the request.
    pub event_request: Signed<EventRequest>,
    /// The state of the request.
    pub state: RequestState,
    /// The success status of the request, if any.
    pub success: Option<bool>,
}

impl TryFrom<Signed<EventRequest>> for KoreRequest {
    type Error = Error;

    fn try_from(
        event_request: Signed<EventRequest>,
    ) -> Result<Self, Self::Error> {
        let id = DigestIdentifier::generate_with_blake3(&event_request)
            .map_err(|_| {
                Error::Digest("Error generation request hash".to_owned())
            })?;
        let subject_id = match &event_request.content {
            EventRequest::Create(_) => None,
            EventRequest::Fact(fact_request) => {
                Some(fact_request.subject_id.clone())
            }
            EventRequest::Transfer(transfer_res) => {
                Some(transfer_res.subject_id.clone())
            }
            EventRequest::EOL(eol_request) => {
                Some(eol_request.subject_id.clone())
            }
        };
        Ok(Self {
            id,
            subject_id,
            sn: None,
            event_request,
            state: RequestState::Processing,
            success: None,
        })
    }
}

#[cfg(test)]
pub mod tests {

    use super::*;

    use identity::{
        identifier::{derive::KeyDerivator, KeyIdentifier},
        keys::{Ed25519KeyPair, KeyGenerator, KeyMaterial, KeyPair},
    };

    // Mocks

    // Create governance request mock.
    pub fn create_start_request_mock(issuer: &str) -> Signed<EventRequest> {
        let (key_pair, key_id) = issuer_identity(issuer);
        let req = StartRequest {
            governance_id: DigestIdentifier::default(),
            schema_id: "governance".to_string(),
            namespace: "namespace".to_string(),
            name: "name".to_string(),
            public_key: key_id,
        };
        let content = EventRequest::Create(req);
        let signature =
            Signature::new(&content, &key_pair, DigestDerivator::SHA2_256)
                .unwrap();
        Signed { content, signature }
    }

    // Mokcs
    #[allow(dead_code)]
    pub fn issuer_identity(name: &str) -> (KeyPair, KeyIdentifier) {
        let filler = [0u8; 32];
        let mut value = name.as_bytes().to_vec();
        value.extend(filler.iter());
        value.truncate(32);
        let kp = Ed25519KeyPair::from_secret_key(&value);
        let id =
            KeyIdentifier::new(KeyDerivator::Ed25519, &kp.public_key_bytes());
        (KeyPair::Ed25519(kp), id)
    }
}
