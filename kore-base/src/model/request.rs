// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Request data model.
//!

use std::collections::HashSet;

use super::{HashId, Namespace, signature::Signed, wrapper::ValueWrapper};

use crate::{
    Error,
    governance::{Governance, model::Roles},
    subject::Metadata,
};

use identity::identifier::{
    DigestIdentifier, KeyIdentifier, derive::digest::DigestDerivator,
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use tracing::error;

const TARGET_REQUEST: &str = "Kore-Model-Request";

pub enum EventRequestType {
    Create,
    Fact,
    Transfer,
    Confirm,
    Reject,
    EOL,
}

pub enum SignerTypes {
    One(KeyIdentifier),
    List(HashSet<KeyIdentifier>, bool),
}

impl From<EventRequest> for EventRequestType {
    fn from(value: EventRequest) -> Self {
        match value {
            EventRequest::Create(_start_request) => Self::Create,
            EventRequest::Fact(_fact_request) => Self::Fact,
            EventRequest::Transfer(_transfer_request) => Self::Transfer,
            EventRequest::Confirm(_confirm_request) => Self::Confirm,
            EventRequest::EOL(_eolrequest) => Self::EOL,
            EventRequest::Reject(_reject_request) => Self::Reject,
        }
    }
}

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
    Create(CreateRequest),
    /// A request to add a fact to a subject.
    Fact(FactRequest),
    /// A request to transfer ownership of a subject.
    Transfer(TransferRequest),

    Confirm(ConfirmRequest),

    Reject(RejectRequest),
    /// A request to mark a subject as end-of-life.
    EOL(EOLRequest),
}

impl EventRequest {
    pub fn check_ledger_signature(
        &self,
        signer: &KeyIdentifier,
        owner: &KeyIdentifier,
        new_owner: &Option<KeyIdentifier>,
    ) -> Result<bool, Error> {
        match self {
            EventRequest::Create(..)
            | EventRequest::Transfer(..)
            | EventRequest::EOL(..) => Ok(signer == owner),
            EventRequest::Confirm(..) | EventRequest::Reject(..) => {
                let Some(new_owner) = new_owner else {
                    return Err(Error::Subject(
                        "new_owner can not be None in Confirm or Reject event"
                            .to_owned(),
                    ));
                };
                Ok(new_owner == signer)
            }
            EventRequest::Fact(..) => Ok(true),
        }
    }

    pub fn check_event_signature(
        &self,
        signer: &KeyIdentifier,
        owner: &KeyIdentifier,
        new_owner: &Option<KeyIdentifier>,
    ) -> Result<bool, Error> {
        match self {
            EventRequest::Create(..)
            | EventRequest::Transfer(..)
            | EventRequest::EOL(..) => Ok(signer == owner),
            EventRequest::Confirm(..) | EventRequest::Reject(..) => {
                let Some(new_owner) = new_owner else {
                    return Err(Error::Subject(
                        "new_owner can not be None in Confirm or Reject event"
                            .to_owned(),
                    ));
                };
                Ok(new_owner == signer)
            }
            EventRequest::Fact(..) => Ok(true),
        }
    }

    pub fn is_create_event(&self) -> bool {
        matches!(self, EventRequest::Create(_create_request))
    }
    pub fn is_fact_event(&self) -> bool {
        matches!(self, EventRequest::Fact(_fact_request))
    }
    pub fn check_signers(
        &self,
        signer: &KeyIdentifier,
        metadata: &Metadata,
        gov: &Governance,
    ) -> bool {
        match self {
            EventRequest::Create(_)
            | EventRequest::EOL(_)
            | EventRequest::Transfer(_) => {
                return metadata.owner == *signer;
            }
            EventRequest::Fact(_) => {
                let (set, any) = gov.get_signers(
                    Roles::ISSUER,
                    &metadata.schema_id,
                    metadata.namespace.clone(),
                );

                if any {
                    return true;
                }

                return set.iter().any(|x| x == signer);
            }
            EventRequest::Confirm(_) | EventRequest::Reject(_) => {
                if let Some(new_owner) = metadata.new_owner.clone() {
                    return new_owner == *signer;
                }
            }
        }
        false
    }
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
pub struct CreateRequest {
    /// The name of subject.
    pub name: Option<String>,
    /// The description of subject.
    pub description: Option<String>,
    /// The identifier of the governance contract.
    pub governance_id: DigestIdentifier,
    /// The identifier of the schema used to validate the event.
    pub schema_id: String,
    /// The namespace of the subject.
    pub namespace: Namespace,
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
pub struct ConfirmRequest {
    pub subject_id: DigestIdentifier,
    /// The new name of old owner, only for governance confirm, if is None in governance confirm, old owner will not add to members
    pub name_old_owner: Option<String>,
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
pub struct RejectRequest {
    pub subject_id: DigestIdentifier,
}

impl HashId for EventRequest {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_REQUEST, "HashId for EventRequest fails: {}", e);
                Error::HashID(format!("HashId for EventRequest fails: {}", e))
            },
        )
    }
}

impl HashId for Signed<EventRequest> {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(
                    TARGET_REQUEST,
                    "HashId for Signed<EventRequest> fails: {}", e
                );
                Error::HashID(format!(
                    "HashId for Signed<EventRequest> fails: {}",
                    e
                ))
            },
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
            .map_err(|e| {
                error!(TARGET_REQUEST, "HashId for KoreRequest fails: {}", e);
                Error::HashID(format!("HashId for KoreRequest fails: {}", e))
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
            EventRequest::Confirm(confirm_request) => {
                Some(confirm_request.subject_id.clone())
            }
            EventRequest::Reject(reject_request) => {
                Some(reject_request.subject_id.clone())
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

    use crate::Signature;

    use super::*;

    use identity::keys::KeyPair;

    // Mocks

    // Create governance request mock.
    pub fn create_start_request_mock(
        _issuer: &str,
        keys: KeyPair,
    ) -> Signed<EventRequest> {
        let req = CreateRequest {
            name: None,
            description: None,
            governance_id: DigestIdentifier::default(),
            schema_id: "governance".to_string(),
            namespace: Namespace::from("namespace"),
        };
        let content = EventRequest::Create(req);
        let signature =
            Signature::new(&content, &keys, DigestDerivator::SHA2_256).unwrap();
        Signed { content, signature }
    }
}
