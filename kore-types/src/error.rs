// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Error module.
//!

use thiserror::Error;

use serde::{Deserialize, Serialize};

/// Error type.
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    /// An error occurred.
    #[error("An error occurred.")]
    Generic,
    /// Actor error.
    #[error("Actor error: {0}")]
    Actor(String),
    /// Store error.
    #[error("Store error: {0}")]
    Store(String),
    /// Digest
    #[error("Digest error: {0}")]
    Digest(String),
    /// Signature
    #[error("Signature error: {0}")]
    Signature(String),
    /// Password
    #[error("Password error: {0}")]
    Password(String),
    /// Request event
    #[error("Request event error: {0}")]
    RequestEvent(String),
    /// Governance error.
    #[error("Governance error: {0}")]
    Governance(String),
    /// Subject error.
    #[error("Subject error: {0}")]
    Subject(String),
    /// Event error.
    #[error("Event error: {0}")]
    Event(String),
    /// Validation error.
    #[error("Validation error: {0}")]
    Validation(String),
    /// Evaluation error.
    #[error("Evaluation error: {0}")]
    Evaluation(String),
    /// Approval error.
    #[error("Approval error: {0}")]
    Approval(String),
}


#[derive(Error, Debug, PartialEq, Clone)]
pub enum SubjectError {
    #[error("Event request type is not Create")]
    NotCreateEvent,
    #[error("Event request type is not State")]
    NotStateEvent,
    #[error("An event is already created waiting to get quorum")]
    EventAlreadyProcessing,
    #[error("An event which is already applied is not in the database")]
    EventAlreadyAppliedNotFound,
    #[error("Event SN is not 0")]
    SnNot0,
    #[error("Event sourcing is not in order")]
    EventSourcingNotInOrder(u64, u64),
    #[error("Hash of Subject_data after apply does not match to event subject_data_hash")]
    EventSourcingHashNotEqual,
    #[error("Applying to Subject without data")]
    ApplyInEmptySubject,
    #[error("Subject Not Found")]
    SubjectNotFound,
    #[error("We are not the owner of the subject")]
    NotOwnerOfSubject,
    #[error("Event Content failed at serialization")]
    EventContentSerializationFailed,
    #[error("Subject Signature Failed")]
    SubjectSignatureFailed,
    #[error("Subject has no data")]
    SubjectHasNoData,
    #[error("Delete Signatures Failed")]
    DeleteSignaturesFailed,
    #[error("Schema Validation Failed")]
    SchemaValidationFailed,
    #[error("Schema does not compile")]
    SchemaDoesNotCompile,
    #[error("Error in criptography")]
    CryptoError(String),
    #[error("InvalidPayload {0}")]
    InvalidPayload(String),
    #[error("Error parsing json string")]
    ErrorParsingJsonString(String),
    #[error("Error applying patch")]
    ErrorApplyingPatch(String),
    #[error("Duplicated schema or member")]
    DuplicatedSchemaOrMember,
    #[error("Policies Missing for Some Schema")]
    PoliciesMissing,
    #[error("Invalid Policies Id")]
    InvalidPoliciesId,
    #[error("Invalid Member in Policies")]
    InvalidMemberInPolicies,
    #[error("Invalid member identifier {0}")]
    InvalidMemberIdentifier(String),
    #[error("JSON-PATCH on Create Event not allowed")]
    InvalidUseOfJSONPATCH,
    #[error("Approvers is not subset of validators")]
    ApproversAreNotValidators,
    #[error("Error creating subject id")]
    ErrorCreatingSubjectId,
    #[error("Signature Creation Fails: {0}")]
    SignatureCreationFails(String),
    #[error("Signature Verify Fails: {0}")]
    SignatureVerifyFails(String),
    #[error("Signature Repeated: {0}")]
    RepeatedSignature(String),
    #[error("Signers Error: {0}")]
    SignersError(String),
}