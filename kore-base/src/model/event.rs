

//! # Event data model.
//!

use super::{
    HashId,
    network::TimeOutResponse,
    request::EventRequest,
    signature::{Signature, Signed},
    wrapper::ValueWrapper,
};

use crate::{Error, subject::Metadata, validation::proof::EventProof};

use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::error;

const TARGET_EVENT: &str = "Kore-Model-Event";

pub struct DataProofEvent {
    pub gov_version: u64,
    pub sn: u64,
    pub metadata: Metadata,
    pub eval_success: Option<bool>,
    pub appr_required: bool,
    pub appr_success: Option<bool>,
    pub value: LedgerValue,
    pub state_hash: DigestIdentifier,
    pub eval_signatures: Option<HashSet<ProtocolsSignatures>>,
    pub appr_signatures: Option<HashSet<ProtocolsSignatures>>,
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
pub struct ProofEvent {
    /// The identifier of the subject of the event.
    pub subject_id: DigestIdentifier,
    /// The type of event proof.
    pub event_proof: EventProof,
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
    pub hash_prev_event: DigestIdentifier,
    /// The set of evaluators who have evaluated the event.
    pub evaluators: Option<HashSet<ProtocolsSignatures>>,
    /// The set of approvers who have approved the event.
    pub approvers: Option<HashSet<ProtocolsSignatures>>,
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
pub enum ProtocolsSignatures {
    Signature(Signature),
    TimeOut(TimeOutResponse),
}

impl HashId for ProofEvent {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_EVENT, "HashId for ProofEvent fails: {}", e);
                Error::HashID(format!("HashId for ProofEvent fails: {}", e))
            },
        )
    }
}

impl HashId for Signed<ProofEvent> {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(
                    TARGET_EVENT,
                    "HashId for Signed<ProofEvent> fails: {}", e
                );
                Error::HashID(format!(
                    "HashId for Signed<ProofEvent> fails: {}",
                    e
                ))
            },
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
    pub evaluators: Option<HashSet<ProtocolsSignatures>>,
    /// The set of approvers who have approved the event.
    pub approvers: Option<HashSet<ProtocolsSignatures>>,

    pub validators: HashSet<ProtocolsSignatures>,
}

impl HashId for Event {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_EVENT, "HashId for Event fails: {}", e);
                Error::HashID(format!("HashId for Event fails: {}", e))
            },
        )
    }
}

impl HashId for Signed<Event> {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_EVENT, "HashId for Signed<Event> fails: {}", e);
                Error::HashID(format!("HashId for Signed<Event> fails: {}", e))
            },
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
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_EVENT, "HashId for Ledger fails: {}", e);
                Error::HashID(format!("HashId for Ledger fails: {}", e))
            },
        )
    }
}

impl HashId for Signed<Ledger> {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_EVENT, "HashId for Signed<Ledger> fails: {}", e);
                Error::HashID(format!("HashId for Signed<Ledger> fails: {}", e))
            },
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
    Error(ProtocolsError),
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
pub struct ProtocolsError {
    pub evaluation: Option<String>,
    pub validation: Option<String>,
}
