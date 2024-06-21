// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Subject module.
//!

use crate::{
    model::{Namespace, ValueWrapper},
    Error,
};

use identity::{
    identifier::{DigestIdentifier, KeyIdentifier},
    keys::{KeyMaterial, KeyPair},
};

use actor::{Actor, ActorContext, Event, Handler, Message, Response};

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use json_patch::{patch, Patch};
use serde::{Deserialize, Serialize};

use std::sync::atomic::{AtomicU64, Ordering};

/// Suject header
#[derive(
    Debug, Serialize, Deserialize
)]
pub struct Subject {
    /// The key pair used to sign the subject.
    keys: KeyPair,
    /// The identifier of the subject.
    subject_id: DigestIdentifier,
    /// The identifier of the governance that drives this subject.
    governance_id: DigestIdentifier,
    /// The namespace of the subject.
    pub namespace: Namespace,
    /// The name of the subject.
    pub name: String,
    /// The identifier of the schema used to validate the subject.
    pub schema_id: String,
    /// The identifier of the public key of the subject owner.
    pub owner: KeyIdentifier,
    /// The identifier of the public key of the subject creator.
    pub creator: KeyIdentifier,
    /// Indicates whether the subject is active or not.
    pub active: bool,
    /// The current sequence number of the subject.
    pub sn: AtomicU64,
    /// The current status of the subject.
    pub properties: ValueWrapper,
}

/// Subject public state.
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SubjectState {
    /// The key identifier of the subject (public key derivate).
    pub subject_key: KeyIdentifier,
    /// The identifier of the subject.
    pub subject_id: DigestIdentifier,
    /// The identifier of the governance that drives this subject.
    pub governance_id: DigestIdentifier,
    /// The namespace of the subject.
    pub namespace: Namespace,
    /// The name of the subject.
    pub name: String,
    /// The identifier of the schema used to validate the subject.
    pub schema_id: String,
    /// The identifier of the public key of the subject owner.
    pub owner: KeyIdentifier,
    /// The identifier of the public key of the subject creator.
    pub creator: KeyIdentifier,
    /// Indicates whether the subject is active or not.
    pub active: bool,
    /// The current sequence number of the subject.
    pub sn: u64,
    /// The current status of the subject.
    pub properties: ValueWrapper,
}

impl From<Subject> for SubjectState {
    fn from(subject: Subject) -> Self {
        Self {
            subject_key: KeyIdentifier::new(
                subject.keys.get_key_derivator(),
                &subject.keys.public_key_bytes(),
            ),
            subject_id: subject.subject_id,
            governance_id: subject.governance_id,
            namespace: subject.namespace,
            name: subject.name,
            schema_id: subject.schema_id,
            owner: subject.owner,
            creator: subject.creator,
            active: subject.active,
            sn: subject.sn.load(Ordering::Relaxed),
            properties: subject.properties,
        }
    }
}

/// Subject command.
#[derive(Debug, Clone)]
pub enum SubjectCommand {
    /// Get the subject.
    GetSubject,
    /// Update the subject.
    UpdateSubject { patch: ValueWrapper, sn: u64 },
}

impl Message for SubjectCommand {}

/// Subject response.
#[derive(Debug, Clone)]
pub enum SubjectResponse {
    /// The subject.
    Subject(SubjectState),
    /// Error.
    Error(Error),
    /// None.
    None,
}

impl Response for SubjectResponse {}

/// Subject event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubjectEvent {
    /// The subject was created.
    Create { subject: SubjectState },
    /// The subject was updated.
    Update { patch: ValueWrapper, sn: u64 },
    /// The subject was deleted.
    Delete { subject_id: DigestIdentifier },
}

impl Event for SubjectEvent {}

/// Actor implementation for `Subject`.
#[async_trait]
impl Actor for Subject {
    type Event = SubjectEvent;
    type Message = SubjectCommand;
    type Response = SubjectResponse;
}

#[async_trait]
impl Handler<Subject> for Subject {
    async fn handle_message(
        &mut self,
        msg: SubjectCommand,
        ctx: &mut ActorContext<Subject>,
    ) -> SubjectResponse {
        match msg {
            SubjectCommand::GetSubject => SubjectResponse::None,
            SubjectCommand::UpdateSubject { patch, sn } => {
                SubjectResponse::None
            }
        }
    }
}
