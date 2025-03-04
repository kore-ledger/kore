// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Data model for Kore base.
//!
//! This module contains the data model for the Kore base.
//!

pub mod common;
pub mod event;
pub mod namespace;
pub mod network;
pub mod patch;
pub mod request;
pub mod signature;
pub mod wrapper;

use event::{Event as KoreEvent, Ledger, ProofEvent};
pub use namespace::Namespace;
pub use wrapper::ValueWrapper;

use crate::{
    Error, EventRequest,
    approval::{
        request::ApprovalReq,
        response::{ApprovalRes, ApprovalSignature},
    },
    evaluation::{request::EvaluationReq, response::EvaluationRes},
    validation::{
        proof::ValidationProof, request::ValidationReq, response::ValidationRes,
    },
};

use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use std::hash::Hash;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SignTypesNode {
    EventRequest(EventRequest),
    Validation(Box<ValidationProof>),
    ValidationProofEvent(ProofEvent),
    ValidationReq(Box<ValidationReq>),
    ValidationRes(ValidationRes),
    EvaluationReq(EvaluationReq),
    EvaluationRes(EvaluationRes),
    ApprovalRes(Box<ApprovalRes>),
    ApprovalReq(ApprovalReq),
    ApprovalSignature(ApprovalSignature),
    Ledger(Ledger),
    Event(KoreEvent),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SignTypesSubject {
    Validation(ValidationProof),
}

/// A trait for generating a hash identifier.
pub trait HashId: BorshSerialize {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error>;
}

/// A struct representing a timestamp.
#[derive(
    Debug,
    Clone,
    Eq,
    PartialEq,
    PartialOrd,
    Ord,
    BorshSerialize,
    BorshDeserialize,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct TimeStamp(pub u64);

impl TimeStamp {
    /// Returns a new `TimeStamp` representing the current time.
    pub fn now() -> Self {
        Self(OffsetDateTime::now_utc().unix_timestamp_nanos() as u64)
    }
}
