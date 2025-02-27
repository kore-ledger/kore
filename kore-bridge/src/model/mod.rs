// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::str::FromStr;

use identity::identifier::{Derivable, DigestIdentifier, KeyIdentifier};
use kore_base::{
    error::Error,
    model::{
        ValueWrapper,
        namespace::Namespace,
        request::{
            ConfirmRequest, CreateRequest, EOLRequest, EventRequest,
            FactRequest, RejectRequest, TransferRequest,
        },
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use signature::BridgeSignature;

mod signature;

/// Signed event request.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BridgeSignedEventRequest {
    /// Event request
    pub request: BridgeEventRequest,
    /// Signature
    pub signature: Option<BridgeSignature>,
}

/// Event request
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BridgeEventRequest {
    Create(BridgeCreateRequest),
    Fact(BridgeFactRequest),
    Transfer(BridgeTransferRequest),
    EOL(BridgeEOLRequest),
    Confirm(BridgeConfirmRequest),
    Reject(BridgeRejectRequest),
}

impl From<EventRequest> for BridgeEventRequest {
    fn from(request: EventRequest) -> Self {
        match request {
            EventRequest::Create(request) => Self::Create(request.into()),
            EventRequest::Fact(request) => Self::Fact(request.into()),
            EventRequest::Transfer(request) => Self::Transfer(request.into()),
            EventRequest::EOL(request) => Self::EOL(request.into()),
            EventRequest::Confirm(request) => Self::Confirm(request.into()),
            EventRequest::Reject(request) => Self::Reject(request.into()),
        }
    }
}

impl TryFrom<BridgeEventRequest> for EventRequest {
    type Error = Error;
    fn try_from(request: BridgeEventRequest) -> Result<Self, Self::Error> {
        match request {
            BridgeEventRequest::Create(request) => {
                Ok(Self::Create(request.try_into()?))
            }
            BridgeEventRequest::Fact(request) => {
                Ok(Self::Fact(request.try_into()?))
            }
            BridgeEventRequest::Transfer(request) => {
                Ok(Self::Transfer(request.try_into()?))
            }
            BridgeEventRequest::EOL(request) => {
                Ok(Self::EOL(request.try_into()?))
            }
            BridgeEventRequest::Confirm(request) => {
                Ok(Self::Confirm(request.try_into()?))
            }
            BridgeEventRequest::Reject(request) => {
                Ok(Self::Reject(request.try_into()?))
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BridgeRejectRequest {
    /// Subject identifier
    pub subject_id: String,
}

impl From<RejectRequest> for BridgeRejectRequest {
    fn from(request: RejectRequest) -> Self {
        Self {
            subject_id: request.subject_id.to_str(),
        }
    }
}

impl TryFrom<BridgeRejectRequest> for RejectRequest {
    type Error = Error;
    fn try_from(request: BridgeRejectRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            subject_id: DigestIdentifier::from_str(&request.subject_id)
                .map_err(|_| {
                    Error::Bridge("Invalid subject identifier".to_string())
                })?,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BridgeCreateRequest {
    /// The identifier of the governance contract.
    pub governance_id: String,
    /// The identifier of the schema used to validate the event.
    pub schema_id: String,
    /// The namespace of the subject.
    pub namespace: String,
}

impl From<CreateRequest> for BridgeCreateRequest {
    fn from(request: CreateRequest) -> Self {
        Self {
            governance_id: request.governance_id.to_str(),
            schema_id: request.schema_id,
            namespace: request.namespace.to_string(),
        }
    }
}

impl TryFrom<BridgeCreateRequest> for CreateRequest {
    type Error = Error;
    fn try_from(request: BridgeCreateRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            governance_id: DigestIdentifier::from_str(&request.governance_id)
                .map_err(|_| {
                Error::Bridge("Invalid governance identifier".to_string())
            })?,
            schema_id: request.schema_id,
            namespace: Namespace::from(request.namespace),
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BridgeFactRequest {
    /// Subject identifier
    pub subject_id: String,
    /// Changes to be applied to the subject
    pub payload: Value,
}

impl From<FactRequest> for BridgeFactRequest {
    fn from(request: FactRequest) -> Self {
        Self {
            subject_id: request.subject_id.to_str(),
            payload: request.payload.0,
        }
    }
}

impl TryFrom<BridgeFactRequest> for FactRequest {
    type Error = Error;
    fn try_from(request: BridgeFactRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            subject_id: DigestIdentifier::from_str(&request.subject_id)
                .map_err(|_| {
                    Error::Bridge("Invalid subject identifier".to_string())
                })?,
            payload: ValueWrapper(request.payload),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeTransferRequest {
    /// Subject identifier
    pub subject_id: String,
    /// Public key of the new owner
    pub new_owner: String,
}

impl From<TransferRequest> for BridgeTransferRequest {
    fn from(request: TransferRequest) -> Self {
        Self {
            subject_id: request.subject_id.to_str(),
            new_owner: request.new_owner.to_str(),
        }
    }
}

impl TryFrom<BridgeTransferRequest> for TransferRequest {
    type Error = Error;
    fn try_from(request: BridgeTransferRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            subject_id: DigestIdentifier::from_str(&request.subject_id)
                .map_err(|_| {
                    Error::Bridge("Invalid subject identifier".to_string())
                })?,
            new_owner: KeyIdentifier::from_str(&request.new_owner)
                .map_err(|_| Error::Bridge("Invalid public key".to_string()))?,
        })
    }
}

/// EOL request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeEOLRequest {
    /// Subject identifier
    pub subject_id: String,
}

impl From<EOLRequest> for BridgeEOLRequest {
    fn from(request: EOLRequest) -> Self {
        Self {
            subject_id: request.subject_id.to_str(),
        }
    }
}

impl TryFrom<BridgeEOLRequest> for EOLRequest {
    type Error = Error;
    fn try_from(request: BridgeEOLRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            subject_id: DigestIdentifier::from_str(&request.subject_id)
                .map_err(|_| {
                    Error::Bridge("Invalid subject identifier".to_string())
                })?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfirmRequest {
    /// Subject identifier
    pub subject_id: String,
    pub name_old_owner: Option<String>,
}

impl From<ConfirmRequest> for BridgeConfirmRequest {
    fn from(request: ConfirmRequest) -> Self {
        Self {
            subject_id: request.subject_id.to_str(),
            name_old_owner: request.name_old_owner,
        }
    }
}

impl TryFrom<BridgeConfirmRequest> for ConfirmRequest {
    type Error = Error;
    fn try_from(request: BridgeConfirmRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            subject_id: DigestIdentifier::from_str(&request.subject_id)
                .map_err(|_| {
                    Error::Bridge("Invalid subject identifier".to_string())
                })?,
            name_old_owner: request.name_old_owner,
        })
    }
}
