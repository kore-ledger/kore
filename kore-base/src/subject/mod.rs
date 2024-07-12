// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Subject module.
//!

use crate::{
    db::{Database, Storable},
    governance,
    model::{
        event::Event as KoreEvent,
        request::{EventRequest, StartRequest},
        signature::Signed,
        Namespace, ValueWrapper,
    },
    Error,
};

use identity::{
    identifier::{
        derive::{digest::DigestDerivator, KeyDerivator},
        DigestIdentifier, KeyIdentifier,
    },
    keys::{
        Ed25519KeyPair, KeyGenerator, KeyMaterial, KeyPair, Secp256k1KeyPair,
    },
};

use actor::{
    Actor, ActorContext, Error as ActorError, Event, Handler, Message, Response,
};

use async_trait::async_trait;
use borsh::{error, BorshDeserialize, BorshSerialize};
use json_patch::{patch, Patch};
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};

use std::sync::atomic::{AtomicU64, Ordering};

/// Suject header
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Subject {
    /// The key pair used to sign the subject.
    keys: KeyPair,
    /// The identifier of the subject.
    subject_id: DigestIdentifier,
    /// The identifier of the governance that drives this subject.
    governance_id: DigestIdentifier,
    /// The governance version.
    governance_version: u64,
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

impl Subject {
    /// Creates a new `Subject` from an create event.
    ///
    /// # Arguments
    ///
    /// * `event` - The event.
    /// * `derivator` - The key derivator.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `Subject` or an `Error`.
    ///
    /// # Errors
    ///
    /// An error is returned if the event is invalid.
    ///
    pub fn from_event(
        subject_keys: KeyPair,
        event: &Signed<KoreEvent>,
    ) -> Result<Self, Error> {
        if let EventRequest::Create(request) =
            &event.content.event_request.content
        {
            let subject = Subject {
                keys: subject_keys,
                subject_id: event.content.subject_id.clone(),
                governance_id: request.governance_id.clone(),
                governance_version: event.content.gov_version,
                namespace: Namespace::from(request.namespace.as_str()),
                name: request.name.clone(),
                schema_id: request.schema_id.clone(),
                owner: event.content.event_request.signature.signer.clone(),
                creator: event.content.event_request.signature.signer.clone(),
                active: true,
                sn: AtomicU64::new(0),
                properties: ValueWrapper::default(),
            };
            Ok(subject)
        } else {
            error!("Invalid create event request");
            Err(Error::Subject("Invalid create event request".to_string()))
        }
    }

    /// Creates subject identifier.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace.
    /// * `schema_id` - The schema identifier.
    /// * `public_key` - The public key identifier.
    /// * `governance_id` - The governance identifier.
    /// * `governance_version` - The governance version.
    /// * `derivator` - The digest derivator.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `DigestIdentifier` or an `Error`.
    ///
    /// # Errors
    ///
    /// An error is returned if the subject identifier cannot be generated.
    ///
    pub fn subject_id(
        namespace: Namespace,
        schema_id: &str,
        public_key: KeyIdentifier,
        governance_id: DigestIdentifier,
        governance_version: u64,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        let subject_id = DigestIdentifier::from_serializable_borsh(
            (
                namespace,
                schema_id,
                public_key,
                governance_id,
                governance_version,
            ),
            derivator,
        )
        .map_err(|_| {
            Error::Subject("Error generating subject id".to_owned())
        })?;
        Ok(subject_id)
    }

    /// Returns subject state.
    ///
    /// # Returns
    ///
    /// The subject state.
    ///
    pub fn state(&self) -> SubjectState {
        SubjectState {
            subject_key: KeyIdentifier::new(
                self.keys.get_key_derivator(),
                &self.keys.public_key_bytes(),
            ),
            subject_id: self.subject_id.clone(),
            governance_id: self.governance_id.clone(),
            namespace: self.namespace.clone(),
            name: self.name.clone(),
            schema_id: self.schema_id.clone(),
            owner: self.owner.clone(),
            creator: self.creator.clone(),
            active: self.active,
            sn: self.sn.load(Ordering::Relaxed),
            properties: self.properties.clone(),
        }
    }

    /// Returns subject metadata.
    ///
    /// # Returns
    ///
    /// The subject metadata.
    ///
    pub fn metadata(&self) -> SubjectMetadata {
        SubjectMetadata {
            subject_id: self.subject_id.clone(),
            governance_id: self.governance_id.clone(),
            governance_version: self.governance_version,
            schema_id: self.schema_id.clone(),
            namespace: self.namespace.clone(),
        }
    }
}

impl Clone for Subject {
    fn clone(&self) -> Self {
        Subject {
            keys: self.keys.clone(),
            subject_id: self.subject_id.clone(),
            governance_id: self.governance_id.clone(),
            governance_version: self.governance_version,
            namespace: self.namespace.clone(),
            name: self.name.clone(),
            schema_id: self.schema_id.clone(),
            owner: self.owner.clone(),
            creator: self.creator.clone(),
            active: self.active,
            sn: AtomicU64::new(self.sn.load(Ordering::Relaxed)),
            properties: self.properties.clone(),
        }
    }
}

/// Subject public state.
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
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

/// Subject metadata.
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub struct SubjectMetadata {
    /// The identifier of the subject of the event.
    pub subject_id: DigestIdentifier,
    /// The identifier of the governance contract.
    pub governance_id: DigestIdentifier,
    /// The version of the governance contract.
    pub governance_version: u64,
    /// The identifier of the schema used to validate the event.
    pub schema_id: String,
    /// The namespace of the subject.
    pub namespace: Namespace,
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
    GetSubjectState,
    /// Get the subject metadata.
    GetSubjectMetadata,
    /// Update the subject.
    UpdateSubject { patch: ValueWrapper, sn: u64 },
}

impl Message for SubjectCommand {}

/// Subject response.
#[derive(Debug, Clone)]
pub enum SubjectResponse {
    /// The subject state.
    SubjectState(SubjectState),
    /// The subject metadata.
    SubjectMetadata(SubjectMetadata),
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

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.init_store(ctx).await
    }

    async fn post_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<Subject> for Subject {
    async fn handle_message(
        &mut self,
        msg: SubjectCommand,
        ctx: &mut ActorContext<Subject>,
    ) -> Result<SubjectResponse, ActorError> {
        match msg {
            SubjectCommand::GetSubjectState => {
                Ok(SubjectResponse::SubjectState(self.state()))
            }
            SubjectCommand::GetSubjectMetadata => {
                Ok(SubjectResponse::SubjectMetadata(self.metadata()))
            }
            SubjectCommand::UpdateSubject { patch, sn } => {
                Ok(SubjectResponse::None)
            }
        }
    }
}

impl PersistentActor for Subject {
    fn apply(&mut self, event: &Self::Event) {
        todo!()
    }
}

impl Storable for Subject {}

#[cfg(test)]
mod tests {

    use super::*;

    use crate::{
        governance::init::init_state,
        model::{
            event::Event as KoreEvent,
            request::tests::create_start_request_mock, signature::Signature,
        },
        tests::create_system,
    };

    #[tokio::test]
    async fn test_subject() {
        let system = create_system().await;
        let request = create_start_request_mock("issuer");
        let keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let event = KoreEvent::from_create_request(
            &keys,
            &request,
            0,
            &init_state(),
            DigestDerivator::Blake3_256,
        )
        .unwrap();
        let signature =
            Signature::new(&event, &keys, DigestDerivator::Blake3_256).unwrap();
        let signed_event = Signed {
            content: event,
            signature,
        };
        let subject = Subject::from_event(keys, &signed_event).unwrap();

        assert_eq!(subject.namespace, Namespace::from("namespace"));
        let actor_id = subject.subject_id.to_string();
        let subject_actor = system
            .create_root_actor(&actor_id, subject.clone())
            .await
            .unwrap();
        let path = subject_actor.path().clone();
        
        let response = subject_actor.ask(SubjectCommand::GetSubjectState).await.unwrap();
        if let SubjectResponse::SubjectState(state) = response {
            assert_eq!(state.namespace, Namespace::from("namespace"));
        } else {
            panic!("Invalid response");
        }
        let response = subject_actor.ask(SubjectCommand::GetSubjectMetadata).await.unwrap();
        if let SubjectResponse::SubjectMetadata(metadata) = response {
            assert_eq!(metadata.namespace, Namespace::from("namespace"));
        } else {
            panic!("Invalid response");
        }

        subject_actor.stop().await;
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let subject_actor = system.get_actor::<Subject>(&path).await;
        assert!(subject_actor.is_none());

        let subject_actor = system
            .create_root_actor(&actor_id, Subject::default())
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let response = subject_actor.ask(SubjectCommand::GetSubjectState).await.unwrap();
        if let SubjectResponse::SubjectState(state) = response {
            assert_eq!(state.namespace, Namespace::from("namespace"));
        } else {
            panic!("Invalid response");
        }
        /*let response = subject_actor.ask(SubjectCommand::GetSubjectMetadata).await.unwrap();
        if let SubjectResponse::SubjectMetadata(metadata) = response {
            assert_eq!(metadata.namespace, Namespace::from("namespace"));
        } else {
            panic!("Invalid response");
        }*/
    
    }
}
