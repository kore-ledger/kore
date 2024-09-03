// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Subject module.
//!

use crate::{
    db::Storable,
    evaluation::{evaluator::Evaluator, schema::EvaluationSchema, Evaluation},
    model::{
        event::Event as KoreEvent,
        request::EventRequest,
        signature::{Signature, Signed},
        HashId, Namespace, SignTypesSubject, ValueWrapper,
    },
    node::{NodeMessage, NodeResponse},
    validation::{schema::ValidationSchema, validator::Validator, Validation},
    Error, Governance, Node, DIGEST_DERIVATOR,
};

use crate::governance::RequestStage;
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};
use identity::{
    identifier::{
        derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
    },
    keys::{KeyMaterial, KeyPair},
};

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use json_patch::{patch, Patch};
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};

use std::{collections::HashSet, str::FromStr, sync::atomic::{AtomicU64, Ordering}};

/// Suject header
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Subject {
    /// The key pair used to sign the subject.
    keys: KeyPair,
    /// The identifier of the subject.
    pub subject_id: DigestIdentifier,
    /// The identifier of the governance that drives this subject.
    pub governance_id: DigestIdentifier,
    /// The version of the governance contract that created the subject.
    pub genesis_gov_version: u64,
    /// The namespace of the subject.
    pub namespace: Namespace,
    /// The name of the subject. TODO: hace falta?
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
                genesis_gov_version: event.content.gov_version,
                namespace: Namespace::from(request.namespace.as_str()),
                name: request.name.clone(),
                schema_id: request.schema_id.clone(),
                owner: event.content.event_request.signature.signer.clone(),
                creator: event.content.event_request.signature.signer.clone(),
                active: true,
                sn: 0,
                properties: event.content.patch.clone(),
            };
            Ok(subject)
        } else {
            error!("Invalid create event request");
            Err(Error::Subject("Invalid create event request".to_string()))
        }
    }

    /// Build a new `Subject` from a subject id with default values.
    ///
    /// # Arguments
    ///
    /// * `subject_id` - The subject identifier.
    ///
    /// # Returns
    ///
    /// A new `Subject` with default values.
    ///
    pub fn from_subject_id(subject_id: DigestIdentifier) -> Self {
        let mut subject = Subject::default();
        subject.with_subject_id(subject_id);
        subject
    }

    async fn get_governace_of_other_subject(
        &self,
        ctx: &mut ActorContext<Subject>,
        subject: DigestIdentifier,
    ) -> Result<Governance, Error> {
        // Governance path
        let governance_path =
            ActorPath::from(format!("/user/node/{}", subject));

        // Governance actor.
        let governance_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&governance_path).await;

        // We obtain the actor governance
        let response = if let Some(governance_actor) = governance_actor {
            // We ask a governance
            // TODO si previous_proof.governance_version == new_proof.governance_version pedimos la governanza, sino tenemos que obtener la versión anterior de governanza
            let response =
                governance_actor.ask(SubjectCommand::GetGovernance).await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a Subject {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The governance actor was not found in the expected path /user/node/{}",
                subject
            )));
        };

        match response {
            SubjectResponse::Governance(gov) => Ok(gov),
            SubjectResponse::Error(error) => {
                return Err(Error::Actor(format!("The subject encountered problems when getting governance: {}",error)));
            }
            _ => {
                return Err(Error::Actor(format!(
                    "An unexpected response has been received from node actor"
                )))
            }
        }
    }

    async fn get_node_key(
        &self,
        ctx: &mut ActorContext<Subject>,
    ) -> Result<KeyIdentifier, Error> {
        // Node path.
        let node_path = ActorPath::from("/user/node");
        // Node actor.
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        // We obtain the actor node
        let response = if let Some(node_actor) = node_actor {
            // We ask a node
            let response =
                node_actor.ask(NodeMessage::GetOwnerIdentifier).await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a node {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The node actor was not found in the expected path /user/node"
            )));
        };

        // We handle the possible responses of node
        match response {
            NodeResponse::OwnerIdentifier(key) => Ok(key),
            _ => Err(Error::Actor(format!(
                "An unexpected response has been received from node actor"
            ))),
        }
    }

    async fn am_i_owner(
        &self,
        ctx: &mut ActorContext<Subject>,
        message: NodeMessage,
    ) -> Result<bool, Error> {
        // Node path.
        let node_path = ActorPath::from("/user/node");
        // Node actor.
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        // We obtain the actor node
        let response = if let Some(node_actor) = node_actor {
            // We ask a node
            let response = node_actor.ask(message).await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a node {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The node actor was not found in the expected path /user/node"
            )));
        };

        // We handle the possible responses of node
        match response {
            NodeResponse::AmIOwner(owner) => Ok(owner),
            _ => Err(Error::Actor(format!(
                "An unexpected response has been received from node actor"
            ))),
        }
    }

    /// Updates the subject with a new subject id.
    ///
    /// # Arguments
    ///
    /// * `subject_id` - The subject identifier.
    ///
    pub fn with_subject_id(&mut self, subject_id: DigestIdentifier) {
        self.subject_id = subject_id;
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
            genesis_gov_version: self.genesis_gov_version,
            namespace: self.namespace.clone(),
            name: self.name.clone(),
            schema_id: self.schema_id.clone(),
            owner: self.owner.clone(),
            creator: self.creator.clone(),
            active: self.active,
            sn: self.sn,
            properties: self.properties.clone(),
        }
    }

    /// Returns subject metadata.
    ///
    /// # Returns
    ///
    /// The subject metadata.
    ///
    fn metadata(&self) -> SubjectMetadata {
        SubjectMetadata {
            subject_id: self.subject_id.clone(),
            governance_id: self.governance_id.clone(),
            schema_id: self.schema_id.clone(),
            namespace: self.namespace.clone(),
            properties: self.properties.clone(),
            sn: self.sn.clone(),
        }
    }

    /// Updates the subject with patch and a new sequence number.
    ///
    /// # Arguments
    ///
    /// * `json_patch` - The json patch.
    /// * `new_sn` - The new sequence number.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `()` or an `Error`.
    ///
    /// # Errors
    ///
    /// An error is returned if the patch cannot be applied.
    ///
    pub fn update_subject(
        &mut self,
        json_patch: ValueWrapper,
        new_sn: u64,
    ) -> Result<(), Error> {
        let Ok(patch_json) = serde_json::from_value::<Patch>(json_patch.0)
        else {
            error!("Subject: Json Patch conversion fails");
            return Err(Error::Subject(
                "Json Patch conversion fails".to_owned(),
            ));
        };
        let Ok(()) = patch(&mut self.properties.0, &patch_json) else {
            error!("Subject: Error Applying Patch");
            return Err(Error::Subject("Error Applying Patch".to_owned()));
        };
        self.sn = new_sn.into();
        Ok(())
    }

    fn sign<T: HashId>(&self, content: &T) -> Result<Signature, Error> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            derivator.clone()
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };

        Signature::new(content, &self.keys, derivator)
            .map_err(|e| Error::Signature(format!("{}", e)))
    }

    async fn build_childs_not_governance(
        &self,
        ctx: &mut ActorContext<Subject>,
    ) -> Result<(), ActorError> {
        // Get node key
        let our_key = self
            .get_node_key(ctx)
            .await
            .map_err(|e| ActorError::Create)?;

        let owner = self
            .am_i_owner(
                ctx,
                NodeMessage::AmISubjectOwner(self.subject_id.clone()),
            )
            .await
            .map_err(|e| ActorError::Create)?;

        if owner {
            let validation = Validation::new(our_key.clone());
            ctx.create_child("validation", validation).await?;

            let evaluation = Evaluation::new(our_key);
            ctx.create_child("evaluation", evaluation).await?;
        }
        Ok(())
    }

    async fn build_childs_governance(
        &self,
        ctx: &mut ActorContext<Subject>,
    ) -> Result<(), ActorError> {
        // Get node key
        let our_key = self
            .get_node_key(ctx)
            .await
            .map_err(|e| ActorError::Create)?;

        // If subject is a governance
        let gov = Governance::try_from(self.state())
            .map_err(|e| ActorError::Create)?;

        let owner = self
            .am_i_owner(
                ctx,
                NodeMessage::AmIGovernanceOwner(self.subject_id.clone()),
            )
            .await
            .map_err(|e| ActorError::Create)?;

        // If we are owner of subject
        if owner {
            let validation = Validation::new(our_key.clone());
            ctx.create_child("validation", validation).await?;

            let evaluation = Evaluation::new(our_key);
            ctx.create_child("evaluation", evaluation).await?;
        } else {
            if self.build_executors(
                RequestStage::Validate,
                &self.schema_id,
                our_key.clone(),
                &gov,
            ) {
                // If we are a validator
                let actor = Validator::default();
                ctx.create_child("validator", actor).await?;
            }

            if self.build_executors(
                RequestStage::Evaluate,
                &self.schema_id,
                our_key,
                &gov,
            ) {
                // If we are a evaluator
                let actor = Evaluator::default();
                ctx.create_child("evaluator", actor).await?;
            }
        }

        let (our_roles, creators) = gov.subjects_schemas_rol_namespace();
        for ((schema, rol), namespaces) in our_roles {
            let mut valid_users =  HashSet::new();
            for ((schema_creator, id), namespaces_creator) in creators.clone() {
                if schema == schema_creator && self.check_namespaces(&namespaces, &namespaces_creator) {
                    if let Ok(id) = KeyIdentifier::from_str(&id) {
                        valid_users.insert(id);
                    }
                }
            }
            match rol {
                crate::governance::model::Roles::APPROVER => {

                },
                crate::governance::model::Roles::EVALUATOR => {
                    let actor = EvaluationSchema::new(valid_users);
                    ctx.create_child(&format!("{}_evaluation", schema), actor).await?;
                },
                crate::governance::model::Roles::VALIDATOR => {
                    let actor = ValidationSchema::new(valid_users);
                    ctx.create_child(&format!("{}_validation", schema), actor).await?;
                },
                _ => {}
            }
        }
        // Hay que tener en cuenta el namespace a la hora de obtener los Validators y Evaluators TODO, por ahora con que tenga el rol lo aceptamos
        // YO puedo ser validador para España.Canarias y recibir una validacion de España.Ceuta, ahí no validar.
        // En principio eso no puede pasar a no ser que haya manipulación de alguien y cambie los namespaces.

        // TODO recorrer los schema de la governanza, usar build_executors para saber si tengo el rol
        // para otros schemas que no sean la governanza y decirle al actor node lo que tiene
        // que crear.

        // Sacar los que tiene el rol de creator.
        Ok(())
    }

    fn check_namespaces(&self, our: &[String], creator: &[String]) -> bool {
        for our_namespace in our {
            let our_namespace = Namespace::from(our_namespace.clone());
            for creator_namespace in creator {
                let creator_namespace =
                    Namespace::from(creator_namespace.clone());
                if our_namespace.is_ancestor_of(&creator_namespace)
                    || our_namespace == creator_namespace || our_namespace.is_empty()
                {
                    return true;
                }
            }
        }
        false
    }

    fn build_executors(
        &self,
        stage: RequestStage,
        schema: &str,
        our_key: KeyIdentifier,
        gov: &Governance,
    ) -> bool {
        gov.get_signers(stage.to_role(), &schema, self.namespace.clone())
            .get(&our_key)
            .is_some()
    }
}

impl Clone for Subject {
    fn clone(&self) -> Self {
        Subject {
            keys: self.keys.clone(),
            subject_id: self.subject_id.clone(),
            governance_id: self.governance_id.clone(),
            genesis_gov_version: self.genesis_gov_version,
            namespace: self.namespace.clone(),
            name: self.name.clone(),
            schema_id: self.schema_id.clone(),
            owner: self.owner.clone(),
            creator: self.creator.clone(),
            active: self.active,
            sn: self.sn,
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
    /// The version of the governance contract that created the subject.
    pub genesis_gov_version: u64,
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
    /// The identifier of the schema used to validate the event.
    pub schema_id: String,
    /// The namespace of the subject.
    pub namespace: Namespace,
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
            genesis_gov_version: subject.genesis_gov_version,
            namespace: subject.namespace,
            name: subject.name,
            schema_id: subject.schema_id,
            owner: subject.owner,
            creator: subject.creator,
            active: subject.active,
            sn: subject.sn,
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
    UpdateSubject {
        event: Signed<KoreEvent>,
    },
    /// Sign request
    SignRequest(SignTypesSubject),
    /// Get governance if subject is a governance
    GetGovernance,
    GetOwner,
}

impl Message for SubjectCommand {}

/// Subject response.
#[derive(Debug, Clone)]
pub enum SubjectResponse {
    /// The subject state.
    SubjectState(SubjectState),
    /// The subject metadata.
    SubjectMetadata(SubjectMetadata),
    SignRequest(Signature),
    Error(Error),
    /// None.
    None,
    Governance(Governance),
    GovernanceId(DigestIdentifier),
    Owner(KeyIdentifier),
}

impl Response for SubjectResponse {}

/// Subject event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubjectEvent {
    /// The subject was updated.
    Update { event: Signed<KoreEvent> },
    /// The subject was patched.
    Patch { value: ValueWrapper },
    /// The subject was deleted.
    Delete { subject_id: DigestIdentifier },
}

impl Event for Signed<KoreEvent> {}

/// Actor implementation for `Subject`.
#[async_trait]
impl Actor for Subject {
    type Event = Signed<KoreEvent>;
    type Message = SubjectCommand;
    type Response = SubjectResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Starting subject actor with init store.");
        self.init_store("subject", true, ctx).await?;

        if self.governance_id.digest.is_empty() {
            self.build_childs_governance(ctx).await?;
        } else {
            self.build_childs_not_governance(ctx).await?;
        }

        Ok(())
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Stopping subject actor with stop store.");
        self.stop_store(ctx).await.map_err(|_| ActorError::Stop)?;
        Ok(())
    }
}

#[async_trait]
impl Handler<Subject> for Subject {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: SubjectCommand,
        ctx: &mut ActorContext<Subject>,
    ) -> Result<SubjectResponse, ActorError> {
        match msg {
            SubjectCommand::GetOwner => {
                Ok(SubjectResponse::Owner(self.owner.clone()))
            }
            SubjectCommand::GetSubjectState => {
                Ok(SubjectResponse::SubjectState(self.state()))
            }
            SubjectCommand::GetSubjectMetadata => {
                Ok(SubjectResponse::SubjectMetadata(self.metadata()))
            }
            SubjectCommand::UpdateSubject { event } => {
                debug!("Emit event to update subject.");
                ctx.event(event).await?;
                Ok(SubjectResponse::None)
            }
            SubjectCommand::SignRequest(content) => {
                let sign = match content {
                    SignTypesSubject::Validation(validation) => {
                        self.sign(&validation)
                    }
                };

                match sign {
                    Ok(sign) => Ok(SubjectResponse::SignRequest(sign)),
                    Err(e) => Ok(SubjectResponse::Error(e)),
                }
            }
            SubjectCommand::GetGovernance => {
                // If a governance
                if self.governance_id.digest.is_empty() {
                    match Governance::try_from(self.state()) {
                        Ok(gov) => return Ok(SubjectResponse::Governance(gov)),
                        Err(e) => return Ok(SubjectResponse::Error(e)),
                    }
                }
                // If not a governance
                Ok(SubjectResponse::Error(Error::Subject(
                    "Subject is not a governance".to_owned(),
                )))
            }
        }
    }

    async fn on_event(
        &mut self,
        event: Signed<KoreEvent>,
        ctx: &mut ActorContext<Subject>,
    ) {
        debug!("Persisting subject event.");
        if let Err(err) = self.persist(&event, ctx).await {
            error!("Error persisting subject event: {:?}", err);
            let _ = ctx.emit_error(err).await;
        };
    }
}

#[async_trait]
impl PersistentActor for Subject {
    fn apply(&mut self, event: &Signed<KoreEvent>) {
        match &event.content.event_request.content {
            EventRequest::Fact(_) => {
                if event.content.approved {
                    debug!("Applying patch to subject: {:?}", self.subject_id);
                    if let Err(e) = self.update_subject(
                        event.content.patch.clone(),
                        event.content.sn,
                    ) {
                        error!("Error applying patch: {:?}", e);
                    }
                } else {
                    self.sn = event.content.sn.into();
                }
            }
            EventRequest::Transfer(_) => {
                // TODO: Implement transfer
                //self.
            }
            EventRequest::EOL(_) => {
                self.sn = event.content.sn.into();
                self.active = false;
            }
            _ => {}
        }
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

    use identity::keys::{Ed25519KeyPair, KeyGenerator};

    #[tokio::test]
    async fn test_subject() {
        let system = create_system().await;
        let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let node = Node::new(&node_keys).unwrap();
        let _ = system.create_root_actor("node", node).await.unwrap();
        let request = create_start_request_mock("issuer");
        let keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let event = KoreEvent::from_create_request(
            &keys,
            &request,
            0,
            &init_state(&node_keys.key_identifier().to_string()),
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
            .get_or_create_actor(
                &format!("node/{}", subject.subject_id),
                || subject.clone(),
            )
            .await
            .unwrap();
        let path = subject_actor.path().clone();

        let response = subject_actor
            .ask(SubjectCommand::GetSubjectState)
            .await
            .unwrap();
        if let SubjectResponse::SubjectState(state) = response {
            assert_eq!(state.namespace, Namespace::from("namespace"));
        } else {
            panic!("Invalid response");
        }
        let response = subject_actor
            .ask(SubjectCommand::GetSubjectMetadata)
            .await
            .unwrap();
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

        let response = subject_actor
            .ask(SubjectCommand::GetSubjectState)
            .await
            .unwrap();
        if let SubjectResponse::SubjectState(state) = response {
            assert_eq!(state.namespace, Namespace::from("namespace"));
        } else {
            panic!("Invalid response");
        }
        let response = subject_actor
            .ask(SubjectCommand::GetSubjectMetadata)
            .await
            .unwrap();
        if let SubjectResponse::SubjectMetadata(metadata) = response {
            assert_eq!(metadata.namespace, Namespace::from("namespace"));
        } else {
            panic!("Invalid response");
        }
    }

    #[test]
    fn test_serialize_deserialize() {
        let value = init_state("");
        let request = create_start_request_mock("issuer");
        let keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let event = KoreEvent::from_create_request(
            &keys,
            &request,
            0,
            &value,
            DigestDerivator::Blake3_256,
        )
        .unwrap();
        let signature =
            Signature::new(&event, &keys, DigestDerivator::Blake3_256).unwrap();
        let signed_event = Signed {
            content: event,
            signature,
        };
        let subject_a = Subject::from_event(keys, &signed_event).unwrap();

        let bytes = bincode::serialize(&subject_a).unwrap();

        let subject_b = bincode::deserialize::<Subject>(&bytes).unwrap();
        assert_eq!(subject_a.subject_id, subject_b.subject_id);
    }
}
