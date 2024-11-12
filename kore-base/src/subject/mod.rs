// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Subject module.
//!

use crate::{
    approval::{approver::Approver, Approval},
    db::Storable,
    distribution::{distributor::Distributor, Distribution},
    evaluation::{
        compiler::{Compiler, CompilerMessage},
        evaluator::Evaluator,
        schema::{EvaluationSchema, EvaluationSchemaMessage},
        Evaluation,
    },
    governance::{model::Roles, Schema},
    helpers::db::LocalDB,
    model::{
        common::{delete_relation, get_gov, get_quantity, register_relation, verify_protocols_state},
        event::{Event as KoreEvent, Ledger, LedgerValue},
        request::EventRequest,
        signature::{Signature, Signed},
        HashId, Namespace, SignTypesSubject, ValueWrapper,
    },
    node::{
        nodekey::{NodeKey, NodeKeyMessage, NodeKeyResponse},
        NodeMessage,
    },
    validation::{
        schema::{ValidationSchema, ValidationSchemaMessage},
        validator::Validator,
        Validation,
    },
    CreateRequest, Error, EventRequestType, Governance, Node, DIGEST_DERIVATOR,
};

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response, Sink,
};
use event::{LedgerEvent, LedgerEventMessage, LedgerEventResponse};
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
use serde_json::to_value;
use store::store::{PersistentActor, Store, StoreCommand, StoreResponse};
use tracing::{debug, error};

use std::{collections::HashSet, str::FromStr};

pub mod event;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateSubjectData {
    pub keys: KeyPair,
    pub create_req: CreateRequest,
    pub subject_id: DigestIdentifier,
    pub creator: KeyIdentifier,
    pub genesis_gov_version: u64,
    pub value: ValueWrapper,
}

/// Subject metadata.
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub struct Metadata {
    /// The identifier of the subject of the event.
    pub subject_id: DigestIdentifier,
    /// The identifier of the governance contract.
    pub governance_id: DigestIdentifier,
    pub genesis_gov_version: u64,
    pub last_event_hash: DigestIdentifier,
    pub subject_public_key: KeyIdentifier,
    /// The identifier of the schema used to validate the event.
    pub schema_id: String,
    /// The namespace of the subject.
    pub namespace: Namespace,
    /// The current sequence number of the subject.
    pub sn: u64,
    /// The identifier of the public key of the subject owner.
    pub owner: KeyIdentifier,
    /// Indicates whether the subject is active or not.
    pub active: bool,
    /// The current status of the subject.
    pub properties: ValueWrapper,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub struct SubjectID {
    // La generamos nosotros
    pub request: Signed<EventRequest>,
    // La generamos nosotros, keypair, derivator (del sujeto) Lo tiene que generar el sujeto
    pub keys: KeyPair,
}

impl HashId for SubjectID {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |_| Error::Evaluation("HashId for ValidationReq fails".to_string()),
        )
    }
}

/// Suject header
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Subject {
    /// The key pair used to sign the subject.
    keys: Option<KeyPair>,
    /// The identifier of the subject.
    pub subject_id: DigestIdentifier,
    /// The identifier of the governance that drives this subject.
    pub governance_id: DigestIdentifier,
    /// The version of the governance contract that created the subject.
    pub genesis_gov_version: u64,
    /// The namespace of the subject.
    pub namespace: Namespace,
    /// The identifier of the schema used to validate the subject.
    pub schema_id: String,
    /// The identifier of the public key of the subject owner.
    pub owner: KeyIdentifier,

    pub last_event_hash: DigestIdentifier,
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
    pub fn new(data: CreateSubjectData) -> Self {
        Subject {
            keys: Some(data.keys),
            subject_id: data.subject_id,
            governance_id: data.create_req.governance_id,
            genesis_gov_version: data.genesis_gov_version,
            namespace: data.create_req.namespace,
            schema_id: data.create_req.schema_id,
            owner: data.creator.clone(),
            creator: data.creator,
            last_event_hash: DigestIdentifier::default(),
            active: true,
            sn: 0,
            properties: data.value,
        }
    }

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
        subject_keys: Option<KeyPair>,
        ledger: &Signed<Ledger>,
    ) -> Result<Self, Error> {
        if let EventRequest::Create(request) =
            &ledger.content.event_request.content
        {
            let properties = if let LedgerValue::Patch(patch) =
                ledger.content.value.clone()
            {
                patch
            } else {
                return Err(Error::Subject(
                    "Invalid create event request".to_string(),
                ));
            };

            let subject = Subject {
                keys: subject_keys,
                subject_id: ledger.content.subject_id.clone(),
                governance_id: request.governance_id.clone(),
                genesis_gov_version: ledger.content.gov_version,
                namespace: request.namespace.clone(),
                schema_id: request.schema_id.clone(),
                last_event_hash: DigestIdentifier::default(),
                owner: ledger.content.event_request.signature.signer.clone(),
                creator: ledger.content.event_request.signature.signer.clone(),
                active: true,
                sn: 0,
                properties,
            };
            Ok(subject)
        } else {
            error!("Invalid create event request");
            Err(Error::Subject("Invalid create event request".to_string()))
        }
    }

    async fn get_node_key(
        &self,
        ctx: &mut ActorContext<Subject>,
    ) -> Result<KeyIdentifier, Error> {
        // Node path.
        let node_key_path = ActorPath::from("/user/node/key");
        // Node actor.
        let node_key_actor: Option<ActorRef<NodeKey>> =
            ctx.system().get_actor(&node_key_path).await;

        // We obtain the actor node
        let response = if let Some(node_key_actor) = node_key_actor {
            // We ask a node
            let response =
                node_key_actor.ask(NodeKeyMessage::GetKeyIdentifier).await;
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
            return Err(Error::Actor(
                "The node actor was not found in the expected path /user/node/key"
                    .to_owned(),
            ));
        };

        // We handle the possible responses of node
        match response {
            NodeKeyResponse::KeyIdentifier(key) => Ok(key),
            _ => Err(Error::Actor(
                "An unexpected response has been received from node actor"
                    .to_owned(),
            )),
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

    /// Returns subject metadata.
    ///
    /// # Returns
    ///
    /// The subject metadata.
    ///
    fn get_metadata(&self) -> Metadata {
        let subject_public_key = if let Some(keys) = self.keys.clone() {
            KeyIdentifier::new(
                keys.get_key_derivator(),
                &keys.public_key_bytes(),
            )
        } else {
            KeyIdentifier::default()
        };

        Metadata {
            subject_public_key,
            genesis_gov_version: self.genesis_gov_version,
            last_event_hash: self.last_event_hash.clone(),
            subject_id: self.subject_id.clone(),
            governance_id: self.governance_id.clone(),
            schema_id: self.schema_id.clone(),
            namespace: self.namespace.clone(),
            properties: self.properties.clone(),
            owner: self.owner.clone(),
            active: self.active,
            sn: self.sn,
        }
    }

    fn sign<T: HashId>(&self, content: &T) -> Result<Signature, Error> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };

        let keys = if let Some(keys) = self.keys.clone() {
            keys
        } else {
            todo!()
        };

        Signature::new(content, &keys, derivator)
            .map_err(|e| Error::Signature(format!("{}", e)))
    }

    async fn build_childs_not_governance(
        &self,
        ctx: &mut ActorContext<Subject>,
        our_key: KeyIdentifier,
    ) -> Result<(), ActorError> {
        let owner = our_key == self.owner;

        if owner {
            Self::up_owner_not_gov(ctx, our_key)
                .await
                .map_err(|e| ActorError::Custom(e.to_string()))?;
        }
        Ok(())
    }

    async fn up_owner_not_gov(
        ctx: &mut ActorContext<Subject>,
        our_key: KeyIdentifier,
    ) -> Result<(), Error> {
        let validation = Validation::new(our_key.clone());
        ctx.create_child("validation", validation)
            .await
            .map_err(|e| Error::Actor(e.to_string()))?;

        let evaluation = Evaluation::new(our_key.clone());
        ctx.create_child("evaluation", evaluation)
            .await
            .map_err(|e| Error::Actor(e.to_string()))?;

        let distribution = Distribution::new(our_key);
        ctx.create_child("distribution", distribution)
            .await
            .map_err(|e| Error::Actor(format!("{}", e)))?;

        Ok(())
    }

    async fn down_owner_not_gov(
        ctx: &mut ActorContext<Subject>,
    ) -> Result<(), Error> {
        let actor: Option<ActorRef<Validation>> =
            ctx.get_child("validation").await;
        if let Some(actor) = actor {
            actor.stop().await;
        } else {
            todo!()
        }

        let actor: Option<ActorRef<Evaluation>> =
            ctx.get_child("evaluation").await;
        if let Some(actor) = actor {
            actor.stop().await;
        } else {
            todo!()
        }

        let actor: Option<ActorRef<Distribution>> =
            ctx.get_child("distribution").await;
        if let Some(actor) = actor {
            actor.stop().await;
        } else {
            todo!()
        }

        Ok(())
    }

    async fn build_childs_governance(
        &self,
        ctx: &mut ActorContext<Subject>,
        our_key: KeyIdentifier,
        local_db: LocalDB
    ) -> Result<(), ActorError> {

        // If subject is a governance
        let gov = Governance::try_from(self.properties.clone())
            .map_err(|e| ActorError::Create)?;

        let owner = our_key == self.owner;

        // If we are owner of subject
        if owner {
            Self::up_owner(
                ctx,
                our_key.clone(),
                self.subject_id.clone(),
                local_db,
            )
            .await
            .map_err(|e| ActorError::Custom(e.to_string()))?;
        } else {
            Self::up_not_owner(
                ctx,
                gov.clone(),
                our_key.clone(),
                self.namespace.clone(),
                local_db,
                self.subject_id.clone(),
            )
            .await
            .map_err(|e| ActorError::Custom(e.to_string()))?;
        }

        let schemas = gov.schemas(Roles::EVALUATOR, &our_key.to_string());
        Self::up_schemas(ctx, schemas, self.subject_id.clone())
            .await
            .map_err(|e| ActorError::Custom(e.to_string()))?;

        let (our_roles, creators) =
            gov.subjects_schemas_rol_namespace(&our_key.to_string());
        for ((schema, rol), namespaces) in our_roles {
            let mut valid_users = HashSet::new();
            for ((schema_creator, id), namespaces_creator) in creators.clone() {
                if schema == schema_creator
                    && self.check_namespaces(&namespaces, &namespaces_creator)
                {
                    if let Ok(id) = KeyIdentifier::from_str(&id) {
                        valid_users.insert(id);
                    }
                }
            }
            match rol {
                crate::governance::model::Roles::EVALUATOR => {
                    let eval_actor = EvaluationSchema::new(valid_users);
                    ctx.create_child(
                        &format!("{}_evaluation", schema),
                        eval_actor,
                    )
                    .await?;
                }
                crate::governance::model::Roles::VALIDATOR => {
                    let actor = ValidationSchema::new(valid_users);
                    ctx.create_child(&format!("{}_validation", schema), actor)
                        .await?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn up_not_owner(
        ctx: &mut ActorContext<Subject>,
        gov: Governance,
        our_key: KeyIdentifier,
        namespace: Namespace,
        local_db: LocalDB,
        subject_id: DigestIdentifier,
    ) -> Result<(), Error> {
        if gov.has_this_role(
            &our_key.to_string(),
            Roles::VALIDATOR,
            "governance",
            namespace.clone(),
        ) {
            // If we are a validator
            let validator = Validator::default();
            ctx.create_child("validator", validator)
                .await
                .map_err(|e| Error::Actor(format!("{}", e)))?;
        }

        if gov.has_this_role(
            &our_key.to_string(),
            Roles::EVALUATOR,
            "governance",
            namespace.clone(),
        ) {
            // If we are a evaluator
            let evaluator = Evaluator::default();
            ctx.create_child("evaluator", evaluator)
                .await
                .map_err(|e| Error::Actor(format!("{}", e)))?;
        }

        if gov.has_this_role(
            &our_key.to_string(),
            Roles::APPROVER,
            "governance",
            namespace.clone(),
        ) {
            // If we are a approver
            let approver = Approver::new(
                "".to_owned(),
                our_key.clone(),
                subject_id.to_string(),
            );
            let approver_actor = ctx
                .create_child("approver", approver)
                .await
                .map_err(|e| Error::Actor(format!("{}", e)))?;

            let sink =
                Sink::new(approver_actor.subscribe(), local_db.get_approver());
            ctx.system().run_sink(sink).await;
        }

        Ok(())
    }

    async fn down_not_owner(
        ctx: &mut ActorContext<Subject>,
        gov: Governance,
        our_key: KeyIdentifier,
        namespace: Namespace,
    ) -> Result<(), Error> {
        if gov.has_this_role(
            &our_key.to_string(),
            Roles::VALIDATOR,
            "governance",
            namespace.clone(),
        ) {
            let actor: Option<ActorRef<Validator>> =
                ctx.get_child("validator").await;
            if let Some(actor) = actor {
                actor.stop().await;
            } else {
                todo!()
            }
        }

        if gov.has_this_role(
            &our_key.to_string(),
            Roles::EVALUATOR,
            "governance",
            namespace.clone(),
        ) {
            let actor: Option<ActorRef<Evaluator>> =
                ctx.get_child("evaluator").await;
            if let Some(actor) = actor {
                actor.stop().await;
            } else {
                todo!()
            }
        }

        if gov.has_this_role(
            &our_key.to_string(),
            Roles::APPROVER,
            "governance",
            namespace.clone(),
        ) {
            let actor: Option<ActorRef<Approver>> =
                ctx.get_child("approver").await;
            if let Some(actor) = actor {
                actor.stop().await;
            } else {
                todo!()
            }
        }

        Ok(())
    }

    async fn up_owner(
        ctx: &mut ActorContext<Subject>,
        our_key: KeyIdentifier,
        subject_id: DigestIdentifier,
        local_db: LocalDB,
    ) -> Result<(), Error> {
        let validation = Validation::new(our_key.clone());
        ctx.create_child("validation", validation)
            .await
            .map_err(|e| Error::Actor(format!("{}", e)))?;

        let evaluation = Evaluation::new(our_key.clone());
        ctx.create_child("evaluation", evaluation)
            .await
            .map_err(|e| Error::Actor(format!("{}", e)))?;

        let approval = Approval::new(our_key.clone());
        ctx.create_child("approval", approval)
            .await
            .map_err(|e| Error::Actor(format!("{}", e)))?;

        let approver = Approver::new(
            "".to_owned(),
            our_key.clone(),
            subject_id.to_string(),
        );
        let approver_actor = ctx
            .create_child("approver", approver)
            .await
            .map_err(|e| Error::Actor(format!("{}", e)))?;

        let sink =
            Sink::new(approver_actor.subscribe(), local_db.get_approver());
        ctx.system().run_sink(sink).await;

        let distribution = Distribution::new(our_key.clone());
        ctx.create_child("distribution", distribution)
            .await
            .map_err(|e| Error::Actor(format!("{}", e)))?;

        Ok(())
    }

    async fn down_owner(ctx: &mut ActorContext<Subject>) -> Result<(), Error> {
        let actor: Option<ActorRef<Validation>> =
            ctx.get_child("validation").await;
        if let Some(actor) = actor {
            actor.stop().await;
        } else {
            todo!()
        }

        let actor: Option<ActorRef<Evaluation>> =
            ctx.get_child("evaluation").await;
        if let Some(actor) = actor {
            actor.stop().await;
        } else {
            todo!()
        }

        let actor: Option<ActorRef<Approval>> = ctx.get_child("approval").await;
        if let Some(actor) = actor {
            actor.stop().await;
        } else {
            todo!()
        }

        let actor: Option<ActorRef<Approver>> = ctx.get_child("approver").await;
        if let Some(actor) = actor {
            actor.stop().await;
        } else {
            todo!()
        }

        let actor: Option<ActorRef<Distribution>> =
            ctx.get_child("distribution").await;
        if let Some(actor) = actor {
            actor.stop().await;
        } else {
            todo!()
        }

        Ok(())
    }

    async fn up_schemas(
        ctx: &mut ActorContext<Subject>,
        schemas: Vec<Schema>,
        subject_id: DigestIdentifier,
    ) -> Result<(), Error> {
        for schema in schemas {
            let actor = Compiler::default();
            let actor = ctx
                .get_or_create_child(&format!("{}_compiler", schema.id), || {
                    actor
                })
                .await
                .map_err(|e| Error::Actor(format!("{}", e)))?;
            if let Err(_e) = actor
                .tell(CompilerMessage::Compile {
                    contract: schema.contract.raw.clone(),
                    initial_value: schema.initial_value.clone(),
                    contract_path: format!("{}_{}", subject_id, schema.id),
                })
                .await
            {
                todo!()
            };
        }

        Ok(())
    }

    async fn down_schemas(
        ctx: &mut ActorContext<Subject>,
        schemas: Vec<Schema>,
    ) -> Result<(), Error> {
        for schema in schemas {
            let actor: Option<ActorRef<Compiler>> =
                ctx.get_child(&format!("{}_compiler", schema.id)).await;
            if let Some(actor) = actor {
                actor.stop().await;
            } else {
                todo!()
            }
        }

        Ok(())
    }

    async fn compile_schemas(
        ctx: &mut ActorContext<Subject>,
        schemas: Vec<Schema>,
        subject_id: DigestIdentifier,
    ) -> Result<(), Error> {
        for schema in schemas {
            let actor: Option<ActorRef<Compiler>> =
                ctx.get_child(&format!("{}_compiler", schema.id)).await;
            if let Some(actor) = actor {
                if let Err(_e) = actor
                    .tell(CompilerMessage::Compile {
                        contract: schema.contract.raw.clone(),
                        initial_value: schema.initial_value.clone(),
                        contract_path: format!("{}_{}", subject_id, schema.id),
                    })
                    .await
                {
                    todo!()
                };
            } else {
                todo!()
            }
        }

        Ok(())
    }

    fn check_namespaces(&self, our: &[String], creator: &[String]) -> bool {
        for our_namespace in our {
            let our_namespace = Namespace::from(our_namespace.clone());
            for creator_namespace in creator {
                let creator_namespace =
                    Namespace::from(creator_namespace.clone());
                if our_namespace.is_ancestor_of(&creator_namespace)
                    || our_namespace == creator_namespace
                    || our_namespace.is_empty()
                {
                    return true;
                }
            }
        }
        false
    }

    async fn get_governance_from_other_subject(
        &self,
        ctx: &mut ActorContext<Subject>,
    ) -> Result<Governance, Error> {
        let governance_path =
            ActorPath::from(format!("/user/node/{}", self.governance_id));

        let governance_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&governance_path).await;

        let response = if let Some(governance_actor) = governance_actor {
            // We ask a governance
            let response =
                governance_actor.ask(SubjectMessage::GetGovernance).await;
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
                "The governance actor was not found in the expected path {}",
                governance_path
            )));
        };

        match response {
            SubjectResponse::Governance(gov) => Ok(gov),
            SubjectResponse::Error(error) => Err(Error::Actor(format!(
                "The subject encountered problems when getting governance: {}",
                error
            ))),
            _ => Err(Error::Actor(
                "An unexpected response has been received from node actor"
                    .to_owned(),
            )),
        }
    }

    async fn get_last_ledger_state(
        &self,
        ctx: &mut ActorContext<Subject>,
    ) -> Result<Option<Signed<Ledger>>, Error> {
        let store: Option<ActorRef<Store<Subject>>> =
            ctx.get_child("store").await;
        let response = if let Some(store) = store {
            match store.ask(StoreCommand::LastEvent).await {
                Ok(response) => response,
                Err(_e) => todo!(),
            }
        } else {
            todo!()
        };

        match response {
            StoreResponse::LastEvent(event) => Ok(event),
            StoreResponse::Error(e) => todo!(),
            _ => todo!(),
        }
    }

    async fn change_node_subject(
        ctx: &mut ActorContext<Subject>,
        subject_id: &str,
        new_owner: &str,
        old_owner: &str,
    ) -> Result<(), Error> {
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        if let Some(node_actor) = node_actor {
            if let Err(_e) = node_actor
                .tell(NodeMessage::ChangeSubjectOwner {
                    new_owner: new_owner.to_owned(),
                    old_owner: old_owner.to_owned(),
                    subject_id: subject_id.to_owned(),
                })
                .await
            {
                todo!()
            }
        } else {
            todo!()
        }
        Ok(())
    }

    async fn verify_new_ledger_event(
        &self,
        last_ledger: &Signed<Ledger>,
        new_ledger: &Signed<Ledger>,
    ) -> Result<(), Error> {
        // Si no sigue activo
        if !self.active {
            todo!();
        }

        // SI no es el dueño el que firmó el evento
        if new_ledger.signature.signer != self.owner {
            todo!();
        }

        if let Err(_e) = new_ledger.verify() {
            todo!()
        }

        // Mirar que sea el siguiente sn
        if last_ledger.content.sn + 1 != new_ledger.content.sn {
            return Err(Error::Sn("Incorrect sn event".to_owned()));
        }

        //Comprobar que el hash del actual event sea el mismo que el pre_event_hash,
        let last_ledger_hash = last_ledger
            .hash_id(last_ledger.signature.content_hash.derivator)?;

        if last_ledger_hash != new_ledger.content.hash_prev_event {
            todo!();
        }

        let valid_last_event = match verify_protocols_state(
            EventRequestType::from(
                last_ledger.content.event_request.content.clone(),
            ),
            last_ledger.content.eval_success,
            last_ledger.content.appr_success,
            last_ledger.content.appr_required,
            last_ledger.content.vali_success,
        ) {
            Ok(is_ok) => is_ok,
            Err(_e) => todo!(),
        };

        // Si el último evento guardado fue correcto, por ende se aplicó lo que ese
        // evento decía.
        if valid_last_event {
            // Comprobar firma,
            if let EventRequest::Transfer(transfer) =
                last_ledger.content.event_request.content.clone()
            {
                if transfer.new_owner != new_ledger.signature.signer {
                    todo!();
                }
            } else if let EventRequest::EOL(end) =
                last_ledger.content.event_request.content.clone()
            {
                // Error, la vida del sujeto terminó y se está registrando un nuevo evento.
                todo!();
            } else if last_ledger.signature.signer
                != new_ledger.signature.signer
            {
                todo!();
            }
        }

        let valid_new_event = match verify_protocols_state(
            EventRequestType::from(
                new_ledger.content.event_request.content.clone(),
            ),
            new_ledger.content.eval_success,
            new_ledger.content.appr_success,
            new_ledger.content.appr_required,
            new_ledger.content.vali_success,
        ) {
            Ok(is_ok) => is_ok,
            Err(_e) => todo!(),
        };

        // Si el nuevo evento a registrar fue correcto.
        if valid_new_event {
            match new_ledger.content.event_request.content.clone() {
                EventRequest::Create(start_request) => {
                    // Error no se puede recibir un evento de creación si ya está creado
                    todo!();
                }
                EventRequest::Fact(fact_request) => {
                    // Al estado actual aplicarle el patch del nuevo evento y ver que obtenemos el mismo hash que el hash del nuevo evento
                    let LedgerValue::Patch(json_patch) =
                        new_ledger.content.value.clone()
                    else {
                        // error el evento fue correcto pero en el value no vino un patch
                        todo!()
                    };

                    let patch_json =
                        serde_json::from_value::<Patch>(json_patch.0)
                            .map_err(|e| todo!())?;
                    let mut propierties = self.properties.0.clone();
                    let Ok(()) = patch(&mut propierties, &patch_json) else {
                        // No se pudo aplicar el patch, error
                        todo!()
                    };

                    let hash_state_after_patch = ValueWrapper(propierties)
                        .hash_id(new_ledger.signature.content_hash.derivator)?;

                    if hash_state_after_patch != new_ledger.content.state_hash {
                        // Error, hemos aplicado el nuevo patch y hemos obtenido un estado diferenta al del nodo original
                    }
                }
                _ => {
                    let hash_without_patch = self
                        .properties
                        .hash_id(new_ledger.signature.content_hash.derivator)?;

                    if hash_without_patch != new_ledger.content.state_hash {
                        // Error, Si el evento no es de fact no se aplicó nungún patch, por ende las dos
                        // propierties deberían ser iguales.
                    }
                }
            };
        }
        // Si el nuevo evento falló en algún protocolo
        else {
            if let LedgerValue::Patch(_) = new_ledger.content.value {
                // Error hay un patch cuando debería haber un error,
            }

            let hash_without_patch = self
                .properties
                .hash_id(new_ledger.signature.content_hash.derivator)?;

            if hash_without_patch != new_ledger.content.state_hash {
                // Error, Si el evento no fue correcto no se aplicó nungún patch, por ende las dos
                // propierties deberían ser iguales.
            }
        }
        Ok(())
    }

    async fn verify_first_ledger_event(
        &self,
        event: Signed<Ledger>,
    ) -> Result<(), Error> {
        if let EventRequest::Create(event_req) =
            event.content.event_request.content.clone()
        {
            if event_req.schema_id == "governance"
                && (!event_req.governance_id.is_empty()
                    || !event_req.namespace.is_empty()
                        && event.content.gov_version != 0)
            {
                todo!()
            }
        } else {
            todo!()
        };

        if event.signature.signer != self.owner
            || event.content.event_request.signature.signer != self.owner
        {
            todo!();
        }

        if let Err(_e) = event.verify() {
            todo!()
        }

        if event.content.sn != 0 {
            todo!()
        }

        if !event.content.hash_prev_event.is_empty() {
            todo!()
        }

        match verify_protocols_state(
            EventRequestType::Create,
            event.content.eval_success,
            event.content.appr_success,
            event.content.appr_required,
            event.content.vali_success,
        ) {
            Ok(is_ok) => {
                if is_ok {
                    Ok(())
                } else {
                    todo!()
                }
            }
            Err(_e) => todo!(),
        }
    }

    async fn verify_new_ledger_events_gov(
        &mut self,
        ctx: &mut ActorContext<Subject>,
        events: &[Signed<Ledger>],
    ) -> Result<(), Error> {
        let mut events = events.to_vec();
        let last_ledger = self.get_last_ledger_state(ctx).await?;

        let mut last_ledger = if let Some(last_ledger) = last_ledger {
            last_ledger
        } else {
            // TODO Hay errores que obligan a borrar el sujeto, como el número máximo de sujetos o un error en la validación del evento de creación.
            if let Err(_e) =
                self.verify_first_ledger_event(events[0].clone()).await
            {
                todo!()
            }

            self.on_event(events[0].clone(), ctx).await;
            events.remove(0)
        };

        for event in events {
            if let Err(e) = self
                .verify_new_ledger_event(&last_ledger, &event)
                .await
            {
                if let Error::Sn(_) = e {
                    // El evento que estamos aplicando no es el siguiente.
                    continue;
                } else {
                    todo!()
                }
            }

            if let EventRequest::Transfer(transfer_request) =
                event.content.event_request.content.clone()
            {
                if let Err(_e) = Subject::change_node_subject(
                    ctx,
                    &transfer_request.subject_id.to_string(),
                    &transfer_request.new_owner.to_string(),
                    &self.owner.to_string(),
                )
                .await
                {
                    todo!()
                }
            };

            // Aplicar evento.
            self.on_event(event.clone(), ctx).await;

            // Acutalizar último evento.
            last_ledger = event.clone();
        }

        Ok(())
    }

    async fn register_relation(
        &self,
        ctx: &mut ActorContext<Subject>,
        signer: String,
        max_quantity: usize,
    ) -> Result<(), Error> {
        if let Err(e) = register_relation(
            ctx,
            self.governance_id.to_string(),
            self.schema_id.clone(),
            signer,
            self.subject_id.to_string(),
            self.namespace.to_string(),
            max_quantity,
        )
        .await
        {
            todo!()
        };

        Ok(())
    }

    async fn verify_new_ledger_events_not_gov(
        &mut self,
        ctx: &mut ActorContext<Subject>,
        events: &[Signed<Ledger>],
    ) -> Result<(), Error> {
        let mut events = events.to_vec();
        let last_ledger = self.get_last_ledger_state(ctx).await?;

        let Ok(gov) = get_gov(ctx, &self.governance_id.to_string()).await
        else {
            todo!()
        };

        let Some(max_quantity) = gov.max_creations(
            &events[0].signature.signer.to_string(),
            &self.schema_id,
            self.namespace.clone(),
        ) else {
            todo!()
        };

        let mut last_ledger = if let Some(last_ledger) = last_ledger {
            last_ledger
        } else {
            // TODO Hay errores que obligan a borrar el sujeto, como el número máximo de sujetos o un error en la validación del evento de creación.
            if let Err(e) = self
                .register_relation(
                    ctx,
                    events[0].signature.signer.to_string(),
                    max_quantity,
                )
                .await
            {
                todo!()
            }

            if let Err(_e) =
                self.verify_first_ledger_event(events[0].clone()).await
            {
                todo!()
            }

            self.on_event(events[0].clone(), ctx).await;
            events.remove(0)
        };

        for event in events {
            if let Err(e) = self
                .verify_new_ledger_event(&last_ledger, &event)
                .await
            {
                if let Error::Sn(_) = e {
                    // El evento que estamos aplicando no es el siguiente.
                    continue;
                } else {
                    todo!()
                }
            }

            if let EventRequest::Transfer(transfer_request) =
                event.content.event_request.content.clone()
            {
                if let Err(e) = delete_relation(
                    ctx,
                    self.governance_id.to_string(),
                    self.schema_id.clone(),
                    event.signature.signer.to_string(),
                    self.subject_id.to_string(),
                    self.namespace.to_string(),
                )
                .await
                {
                    todo!()
                }

                if let Err(_e) = Subject::change_node_subject(
                    ctx,
                    &transfer_request.subject_id.to_string(),
                    &transfer_request.new_owner.to_string(),
                    &self.owner.to_string(),
                )
                .await
                {
                    todo!()
                }
            } else if let EventRequest::Confirm(confirm_request) =
                event.content.event_request.content.clone()
            {
                if let Err(e) = self
                    .register_relation(
                        ctx,
                        event.signature.signer.to_string(),
                        max_quantity,
                    )
                    .await
                {
                    todo!()
                }
            };

            // Aplicar evento.
            self.on_event(event.clone(), ctx).await;

            // Acutalizar último evento.
            last_ledger = event.clone();
        }

        Ok(())
    }

    async fn verify_new_ledger_events(
        &mut self,
        ctx: &mut ActorContext<Subject>,
        events: &[Signed<Ledger>],
    ) -> Result<(), Error> {
        let our_key = self.get_node_key(ctx).await?;

        if self.governance_id.is_empty() {
            let current_owner = self.owner.clone();
            let current_sn = self.sn;
            let current_properties = self.properties.clone();

            if let Err(e) = self.verify_new_ledger_events_gov(ctx, events).await
            {
                // Hay que ver el fallo TODO, porque se pudieron aplicar X eventos y hay que realizar acutalizaciones.
            };

            if current_sn < self.sn {
                let old_gov = Governance::try_from(current_properties)?;
                if !self.active {
                    if current_owner == our_key {
                        Self::down_owner(ctx).await?;
                    } else {
                        Self::down_not_owner(
                            ctx,
                            old_gov.clone(),
                            our_key.clone(),
                            self.namespace.clone(),
                        )
                        .await?;
                    }

                    let old_schemas =
                        old_gov.schemas(Roles::EVALUATOR, &our_key.to_string());
                    Self::down_schemas(ctx, old_schemas.clone()).await?;

                    let old_schemas_val = old_gov
                        .schemas(Roles::VALIDATOR, &our_key.to_string())
                        .iter()
                        .map(|x| x.id.clone())
                        .collect::<Vec<String>>();
                    let old_schemas_eval = old_schemas
                        .iter()
                        .map(|x| x.id.clone())
                        .collect::<Vec<String>>();

                    for schema in old_schemas_eval {
                        let actor: Option<ActorRef<EvaluationSchema>> = ctx
                            .get_child(&format!("{}_evaluation", schema))
                            .await;
                        if let Some(actor) = actor {
                            actor.stop().await;
                        } else {
                            todo!()
                        }
                    }

                    for schema in old_schemas_val {
                        let actor: Option<ActorRef<ValidationSchema>> = ctx
                            .get_child(&format!("{}_validation", schema))
                            .await;
                        if let Some(actor) = actor {
                            actor.stop().await;
                        } else {
                            todo!()
                        }
                    }
                } else {
                    let new_gov =
                        Governance::try_from(self.properties.clone())?;

                    // Si cambió el dueño
                    if current_owner != self.owner {
                        let Some(local_db): Option<LocalDB> =
                            ctx.system().get_helper("local_db").await
                        else {
                            todo!()
                        };

                        if self.owner == our_key {
                            // Ahora somos el dueño
                            Self::down_not_owner(
                                ctx,
                                old_gov.clone(),
                                our_key.clone(),
                                self.namespace.clone(),
                            )
                            .await?;
                            Self::up_owner(
                                ctx,
                                our_key.clone(),
                                self.subject_id.clone(),
                                local_db,
                            )
                            .await?;
                        } else if current_owner == our_key {
                            // Antes era el dueño y ahora no.
                            Self::down_owner(ctx).await?;
                            Self::up_not_owner(
                                ctx,
                                new_gov.clone(),
                                our_key.clone(),
                                self.namespace.clone(),
                                local_db,
                                self.subject_id.clone(),
                            )
                            .await?;
                        }
                    }

                    let old_schemas =
                        old_gov.schemas(Roles::EVALUATOR, &our_key.to_string());
                    let new_schemas =
                        new_gov.schemas(Roles::EVALUATOR, &our_key.to_string());

                    // Bajamos los compilers que ya no soy evaluador
                    let mut down = old_schemas.clone();
                    down.retain(|x| !new_schemas.contains(x));
                    Self::down_schemas(ctx, down).await?;

                    // Subimos los compilers que soy nuevo evaluador
                    let mut up = new_schemas.clone();
                    up.retain(|x| !old_schemas.contains(x));
                    Self::up_schemas(ctx, up, self.subject_id.clone()).await?;

                    // Compilo los nuevos contratos en el caso de que hayan sido modificados, sino no afecta.
                    let mut current = old_schemas.clone();
                    current.retain(|x| new_schemas.contains(x));
                    Self::compile_schemas(
                        ctx,
                        current,
                        self.subject_id.clone(),
                    )
                    .await?;

                    let mut old_schemas_val = old_gov
                        .schemas(Roles::VALIDATOR, &our_key.to_string())
                        .iter()
                        .map(|x| x.id.clone())
                        .collect::<Vec<String>>();
                    let mut old_schemas_eval = old_schemas
                        .iter()
                        .map(|x| x.id.clone())
                        .collect::<Vec<String>>();

                    let (our_roles, creators) = new_gov
                        .subjects_schemas_rol_namespace(&our_key.to_string());
                    for ((schema, rol), namespaces) in our_roles {
                        let mut valid_users = HashSet::new();
                        for ((schema_creator, id), namespaces_creator) in
                            creators.clone()
                        {
                            if schema == schema_creator
                                && self.check_namespaces(
                                    &namespaces,
                                    &namespaces_creator,
                                )
                            {
                                if let Ok(id) = KeyIdentifier::from_str(&id) {
                                    valid_users.insert(id);
                                }
                            }
                        }
                        match rol {
                            crate::governance::model::Roles::EVALUATOR => {
                                let pos = old_schemas_eval
                                    .iter()
                                    .position(|x| *x == schema);
                                if let Some(pos) = pos {
                                    old_schemas_eval.remove(pos);
                                    let actor: Option<
                                        ActorRef<EvaluationSchema>,
                                    > = ctx
                                        .get_child(&format!(
                                            "{}_evaluation",
                                            schema
                                        ))
                                        .await;
                                    if let Some(actor) = actor {
                                        if let Err(e) = actor.tell(EvaluationSchemaMessage::UpdateEvaluators(valid_users)).await {todo!()}
                                    } else {
                                        todo!()
                                    }
                                } else {
                                    let eval_actor =
                                        EvaluationSchema::new(valid_users);
                                    ctx.create_child(
                                        &format!("{}_evaluation", schema),
                                        eval_actor,
                                    )
                                    .await
                                    .map_err(|e| Error::Actor(e.to_string()))?;
                                }
                            }
                            crate::governance::model::Roles::VALIDATOR => {
                                let pos = old_schemas_val
                                    .iter()
                                    .position(|x| *x == schema);
                                if let Some(pos) = pos {
                                    old_schemas_val.remove(pos);
                                    let actor: Option<
                                        ActorRef<ValidationSchema>,
                                    > = ctx
                                        .get_child(&format!(
                                            "{}_validation",
                                            schema
                                        ))
                                        .await;
                                    if let Some(actor) = actor {
                                        if let Err(e) = actor.tell(ValidationSchemaMessage::UpdateValidators(valid_users)).await {todo!()}
                                    } else {
                                        todo!()
                                    }
                                } else {
                                    let actor =
                                        ValidationSchema::new(valid_users);
                                    ctx.create_child(
                                        &format!("{}_validation", schema),
                                        actor,
                                    )
                                    .await
                                    .map_err(|e| Error::Actor(e.to_string()))?;
                                }
                            }
                            _ => {}
                        }
                    }

                    for schema in old_schemas_eval {
                        let actor: Option<ActorRef<EvaluationSchema>> = ctx
                            .get_child(&format!("{}_evaluation", schema))
                            .await;
                        if let Some(actor) = actor {
                            actor.stop().await;
                        } else {
                            todo!()
                        }
                    }

                    for schema in old_schemas_val {
                        let actor: Option<ActorRef<ValidationSchema>> = ctx
                            .get_child(&format!("{}_validation", schema))
                            .await;
                        if let Some(actor) = actor {
                            actor.stop().await;
                        } else {
                            todo!()
                        }
                    }
                }
            }
        } else {
            let current_owner = self.owner.clone();
            let current_sn = self.sn;

            if let Err(e) =
                self.verify_new_ledger_events_not_gov(ctx, events).await
            {
                // Hay que ver el fallo TODO, porque se pudieron aplicar X eventos y hay que realizar acutalizaciones.
            };

            if current_sn < self.sn {
                if !self.active {
                    if current_owner == our_key {
                        Self::down_owner_not_gov(ctx).await?;
                    }
                }

                if current_owner != self.owner {
                    if self.owner == our_key {
                        Self::up_owner_not_gov(ctx, our_key).await?;
                    } else if current_owner == our_key {
                        Self::down_owner_not_gov(ctx).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn get_ledger(
        &self,
        ctx: &mut ActorContext<Subject>,
        last_sn: u64,
    ) -> Result<Vec<Signed<Ledger>>, Error> {
        let store: Option<ActorRef<Store<Subject>>> =
            ctx.get_child("store").await;
        let response = if let Some(store) = store {
            match store
                .ask(StoreCommand::GetEvents {
                    from: last_sn,
                    to: last_sn + 100,
                })
                .await
            {
                Ok(response) => response,
                Err(_e) => todo!(),
            }
        } else {
            todo!()
        };

        match response {
            StoreResponse::Events(events) => Ok(events),
            _ => todo!(),
        }
    }

    async fn get_last_event(
        &self,
        ctx: &mut ActorContext<Subject>,
    ) -> Result<Signed<KoreEvent>, Error> {
        let ledger_event_path = ActorPath::from(format!(
            "/user/node/{}/ledger_event",
            self.subject_id
        ));
        let ledger_event_actor: Option<ActorRef<LedgerEvent>> =
            ctx.system().get_actor(&ledger_event_path).await;

        let response = if let Some(ledger_event_actor) = ledger_event_actor {
            if let Ok(response) = ledger_event_actor
                .ask(LedgerEventMessage::GetLastEvent)
                .await
            {
                response
            } else {
                todo!()
            }
        } else {
            todo!()
        };

        match response {
            LedgerEventResponse::LastEvent(event) => Ok(event),
            _ => todo!(),
        }
    }

    async fn create_compilers(
        ctx: &mut ActorContext<Subject>,
        compilers: &[String],
    ) -> Result<Vec<String>, Error> {
        let mut new_compilers = vec![];

        for compiler in compilers {
            let child: Option<ActorRef<Compiler>> =
                ctx.get_child(&format!("{}_compiler", compiler)).await;
            if child.is_none() {
                new_compilers.push(compiler.clone());
                if let Err(e) = ctx
                    .create_child(
                        &format!("{}_compiler", compiler),
                        Compiler::default(),
                    )
                    .await
                {
                    todo!()
                }
            }
        }

        Ok(new_compilers)
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
            schema_id: self.schema_id.clone(),
            last_event_hash: self.last_event_hash.clone(),
            owner: self.owner.clone(),
            creator: self.creator.clone(),
            active: self.active,
            sn: self.sn,
            properties: self.properties.clone(),
        }
    }
}

/// Subject command.
#[derive(Debug, Clone)]
pub enum SubjectMessage {
    CreateCompilers(Vec<String>),
    /// Get the subject metadata.
    GetMetadata,
    GetLedger {
        last_sn: u64,
    },
    UpdateLedger {
        events: Vec<Signed<Ledger>>,
    },
    /// Sign request
    SignRequest(Box<SignTypesSubject>),
    /// Get governance if subject is a governance
    GetGovernance,
    GetOwner,
}

impl Message for SubjectMessage {}

/// Subject response.
#[derive(Debug, Clone)]
pub enum SubjectResponse {
    /// The subject metadata.
    Metadata(Metadata),
    SignRequest(Signature),
    Error(Error),
    LastSn(u64),
    Ledger((Vec<Signed<Ledger>>, Option<Signed<KoreEvent>>)),
    Governance(Governance),
    Owner(KeyIdentifier),
    NewCompilers(Vec<String>),
}

impl Response for SubjectResponse {}

impl Event for Signed<Ledger> {}

/// Actor implementation for `Subject`.
#[async_trait]
impl Actor for Subject {
    type Event = Signed<Ledger>;
    type Message = SubjectMessage;
    type Response = SubjectResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Starting subject actor with init store.");
        self.init_store("subject", None, true, ctx).await?;

        let our_key = self
            .get_node_key(ctx)
            .await
            .map_err(|e| ActorError::Create)?;

        let Some(local_db): Option<LocalDB> =
            ctx.system().get_helper("local_db").await
        else {
            todo!()
        };

        let ledger_event = LedgerEvent::new(self.governance_id.is_empty());
        let ledger_event_actor = ctx.create_child("ledger_event", ledger_event).await?;

        let sink =
        Sink::new(ledger_event_actor.subscribe(), local_db.get_ledger_event());
        ctx.system().run_sink(sink).await;

        if self.active {
            if self.governance_id.is_empty() {
                self.build_childs_governance(ctx, our_key.clone(), local_db).await?;
            } else {
                self.build_childs_not_governance(ctx, our_key.clone())
                    .await?;
            }
        }

        let distributor = Distributor { node: our_key };
        ctx.create_child("distributor", distributor).await?;

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
        _sender: ActorPath,
        msg: SubjectMessage,
        ctx: &mut ActorContext<Subject>,
    ) -> Result<SubjectResponse, ActorError> {
        match msg {
            SubjectMessage::CreateCompilers(compilers) => {
                match Self::create_compilers(ctx, &compilers).await {
                    Ok(new_compilers) => {
                        Ok(SubjectResponse::NewCompilers(new_compilers))
                    }
                    Err(e) => Ok(SubjectResponse::Error(e)),
                }
            }
            SubjectMessage::GetLedger { last_sn } => {
                let ledger = match self.get_ledger(ctx, last_sn).await {
                    Ok(response) => response,
                    Err(e) => return Ok(SubjectResponse::Error(e)),
                };

                if ledger.len() < 100 {
                    match self.get_last_event(ctx).await {
                        Ok(last_event) => Ok(SubjectResponse::Ledger((
                            ledger,
                            Some(last_event),
                        ))),
                        Err(e) => Ok(SubjectResponse::Error(e)),
                    }
                } else {
                    Ok(SubjectResponse::Ledger((ledger, None)))
                }
            }
            SubjectMessage::GetOwner => {
                Ok(SubjectResponse::Owner(self.owner.clone()))
            }
            SubjectMessage::GetMetadata => {
                Ok(SubjectResponse::Metadata(self.get_metadata()))
            }
            SubjectMessage::UpdateLedger { events } => {
                debug!("Emit event to update subject.");
                if let Err(e) =
                    self.verify_new_ledger_events(ctx, events.as_slice()).await
                {
                    Ok(SubjectResponse::Error(e))
                } else {
                    Ok(SubjectResponse::LastSn(self.sn))
                }
            }
            SubjectMessage::SignRequest(content) => {
                let sign = match *content {
                    SignTypesSubject::Validation(validation) => {
                        self.sign(&validation)
                    }
                };

                match sign {
                    Ok(sign) => Ok(SubjectResponse::SignRequest(sign)),
                    Err(e) => Ok(SubjectResponse::Error(e)),
                }
            }
            SubjectMessage::GetGovernance => {
                // If is a governance
                if self.governance_id.is_empty() {
                    match Governance::try_from(self.properties.clone()) {
                        Ok(gov) => return Ok(SubjectResponse::Governance(gov)),
                        Err(e) => return Ok(SubjectResponse::Error(e)),
                    }
                }
                // If is not a governance
                match self.get_governance_from_other_subject(ctx).await {
                    Ok(gov) => return Ok(SubjectResponse::Governance(gov)),
                    Err(e) => return Ok(SubjectResponse::Error(e)),
                }
            }
        }
    }

    async fn on_event(
        &mut self,
        event: Signed<Ledger>,
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
    fn apply(&mut self, event: &Signed<Ledger>) {
        let valid_event = match verify_protocols_state(
            EventRequestType::from(event.content.event_request.content.clone()),
            event.content.eval_success,
            event.content.appr_success,
            event.content.appr_required,
            event.content.vali_success,
        ) {
            Ok(is_ok) => is_ok,
            Err(_e) => todo!(),
        };

        if valid_event {
            match &event.content.event_request.content {
                EventRequest::Create(start_request) => {
                    let last_event_hash = match event
                        .hash_id(event.signature.content_hash.derivator)
                    {
                        Ok(hash) => hash,
                        Err(_e) => todo!(),
                    };

                    self.last_event_hash = last_event_hash;
                    return;
                }
                EventRequest::Fact(fact_request) => {
                    let json_patch = match event.content.value.clone() {
                        LedgerValue::Patch(value_wrapper) => value_wrapper,
                        LedgerValue::Error(e) => todo!(),
                    };

                    let patch_json =
                        match serde_json::from_value::<Patch>(json_patch.0) {
                            Ok(patch) => patch,
                            Err(_e) => todo!(),
                        };

                    if let Err(_e) = patch(&mut self.properties.0, &patch_json)
                    {
                        // No se pudo aplicar el patch, error
                        todo!()
                    };
                }
                EventRequest::Transfer(transfer_request) => {
                    // TODO hay que darle una vuelta.
                    self.owner = transfer_request.new_owner.clone();
                }
                EventRequest::EOL(eolrequest) => self.active = false,
                EventRequest::Confirm(confirm_request) => {
                    todo!()
                }
            }

            if self.governance_id.is_empty() {
                let mut gov =
                    match Governance::try_from(self.properties.clone()) {
                        Ok(gov) => gov,
                        Err(_e) => todo!(),
                    };

                gov.version += 1;
                let gov_value = if let Ok(value) = to_value(gov) {
                    value
                } else {
                    todo!()
                };

                self.properties.0 = gov_value;
            }
        }

        let last_event_hash =
            match event.hash_id(event.signature.content_hash.derivator) {
                Ok(hash) => hash,
                Err(_e) => todo!(),
            };

        self.last_event_hash = last_event_hash;

        self.sn += 1;
    }
}

impl Storable for Subject {}

#[cfg(test)]
mod tests {

    use std::time::{Duration, Instant};

    use super::*;

    use crate::{
        governance::init::init_state,
        model::{
            event::Event as KoreEvent,
            request::tests::create_start_request_mock, signature::Signature,
        },
        system::tests::create_system,
        FactRequest,
    };

    async fn create_subject_and_ledger_event(
        system: SystemRef,
        node_keys: KeyPair,
    ) -> (
        ActorRef<Subject>,
        ActorRef<LedgerEvent>,
        Subject,
        Signed<Ledger>,
    ) {
        let node = Node::new(&node_keys).unwrap();
        let _ = system.create_root_actor("node", node).await.unwrap();
        let request = create_start_request_mock("issuer", node_keys.clone());
        let keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let event = KoreEvent::from_create_request(
            &keys,
            &request,
            0,
            &init_state(&node_keys.key_identifier().to_string()),
            DigestDerivator::Blake3_256,
        )
        .unwrap();
        let ledger = Ledger::from(event.clone());
        let signature_ledger = Signature::new(
            &ledger,
            &node_keys.clone(),
            DigestDerivator::Blake3_256,
        )
        .unwrap();
        let signed_ledger = Signed {
            content: ledger,
            signature: signature_ledger,
        };

        let signature_event =
            Signature::new(&event, &node_keys, DigestDerivator::Blake3_256)
                .unwrap();

        let signed_event = Signed {
            content: event,
            signature: signature_event,
        };

        let subject =
            Subject::from_event(Some(keys.clone()), &signed_ledger).unwrap();

        let subject_actor = system
            .get_or_create_actor(
                &format!("node/{}", subject.subject_id),
                || subject.clone(),
            )
            .await
            .unwrap();

        let ledger_event_actor: Option<ActorRef<LedgerEvent>> = system
            .get_actor(&ActorPath::from(format!(
                "user/node/{}/ledger_event",
                subject.subject_id
            )))
            .await;

        let ledger_event_actor = if let Some(actor) = ledger_event_actor {
            actor
        } else {
            panic!("Actor must be in system actor")
        };

        ledger_event_actor
            .ask(LedgerEventMessage::UpdateLastEvent {
                event: signed_event,
            })
            .await
            .unwrap();

        let response = subject_actor
            .ask(SubjectMessage::UpdateLedger {
                events: vec![signed_ledger.clone()],
            })
            .await
            .unwrap();

        if let SubjectResponse::LastSn(last_sn) = response {
            assert_eq!(last_sn, 0);
        } else {
            panic!("Invalid response");
        }

        (subject_actor, ledger_event_actor, subject, signed_ledger)
    }

    fn create_n_fact_events(
        mut hash_prev_event: DigestIdentifier,
        n: u64,
        keys: KeyPair,
        subject_id: DigestIdentifier,
        mut subject_propierties: Value,
    ) -> Vec<Signed<Ledger>> {
        let mut vec: Vec<Signed<Ledger>> = vec![];

        for i in 0..n {
            let patch_event_req = json!(
                    [
                        {
                            "op": "add",
                            "path": "/members/0",
                            "value": {
                                "id": KeyIdentifier::new(KeyDerivator::Ed25519, &vec![]),
                                "name": format!("KoreNode{}", i)
                            }
                        }
                    ]
            );

            let event_req = EventRequest::Fact(FactRequest {
                subject_id: subject_id.clone(),
                payload: ValueWrapper(patch_event_req.clone()),
            });

            let signature_event_req =
                Signature::new(&event_req, &keys, DigestDerivator::Blake3_256)
                    .unwrap();

            let signed_event_req = Signed {
                content: event_req,
                signature: signature_event_req,
            };

            let patch_json =
                serde_json::from_value::<Patch>(patch_event_req.clone())
                    .unwrap();
            patch(&mut subject_propierties, &patch_json).unwrap();

            let state_hash = ValueWrapper(subject_propierties.clone())
                .hash_id(DigestDerivator::Blake3_256)
                .unwrap();

            let ledger = Ledger {
                subject_id: subject_id.clone(),
                event_request: signed_event_req,
                sn: i + 1,
                gov_version: i,
                value: LedgerValue::Patch(ValueWrapper(patch_event_req)),
                state_hash,
                eval_success: Some(true),
                appr_required: false,
                appr_success: None,
                vali_success: true,
                hash_prev_event: hash_prev_event.clone(),
            };

            let signature_ledger =
                Signature::new(&ledger, &keys, DigestDerivator::Blake3_256)
                    .unwrap();

            let signed_ledger = Signed {
                content: ledger,
                signature: signature_ledger,
            };

            hash_prev_event =
                signed_ledger.hash_id(DigestDerivator::Blake3_256).unwrap();
            vec.push(signed_ledger);
        }

        vec
    }

    impl KoreEvent {
        pub fn from_create_request(
            subject_keys: &KeyPair,
            request: &Signed<EventRequest>,
            governance_version: u64,
            init_state: &ValueWrapper,
            derivator: DigestDerivator,
        ) -> Result<Self, Error> {
            let EventRequest::Create(start_request) = &request.content else {
                return Err(Error::Event("Invalid Event Request".to_string()));
            };
            let public_key = KeyIdentifier::new(
                subject_keys.get_key_derivator(),
                &subject_keys.public_key_bytes(),
            );

            let subject_id = Subject::subject_id(
                start_request.namespace.clone(),
                &start_request.schema_id,
                public_key,
                start_request.governance_id.clone(),
                governance_version,
                derivator,
            )?;
            let state_hash = DigestIdentifier::from_serializable_borsh(
                init_state, derivator,
            )
            .map_err(|_| {
                Error::Digest("Error converting state to hash".to_owned())
            })?;

            Ok(KoreEvent {
                subject_id,
                event_request: request.clone(),
                sn: 0,
                gov_version: governance_version,
                value: LedgerValue::Patch(init_state.clone()),
                state_hash,
                eval_success: None,
                appr_required: false,
                hash_prev_event: DigestIdentifier::default(),
                evaluators: None,
                approvers: None,
                appr_success: None,
                vali_success: true,
                validators: HashSet::new(),
            })
        }
    }

    use actor::SystemRef;
    use identity::{
        identifier::derive::KeyDerivator,
        keys::{Ed25519KeyPair, KeyGenerator},
    };
    use serde_json::{json, Value};

    #[tokio::test]
    async fn test_subject() {
        let system = create_system().await;
        let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let node = Node::new(&node_keys).unwrap();
        let _ = system.create_root_actor("node", node).await.unwrap();
        let request = create_start_request_mock("issuer", node_keys.clone());
        let keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let event = KoreEvent::from_create_request(
            &keys,
            &request,
            0,
            &init_state(&node_keys.key_identifier().to_string()),
            DigestDerivator::Blake3_256,
        )
        .unwrap();
        let ledger = Ledger::from(event);
        let signature =
            Signature::new(&ledger, &node_keys, DigestDerivator::Blake3_256)
                .unwrap();
        let signed_ledger = Signed {
            content: ledger,
            signature,
        };

        let subject = Subject::from_event(Some(keys), &signed_ledger).unwrap();

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
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap();
        if let SubjectResponse::Metadata(metadata) = response {
            assert_eq!(metadata.namespace, Namespace::from("namespace"));
        } else {
            panic!("Invalid response");
        }

        subject_actor.stop().await;
        tokio::time::sleep(Duration::from_secs(1)).await;

        let subject_actor = system.get_actor::<Subject>(&path).await;
        assert!(subject_actor.is_none());

        let subject_actor = system
            .create_root_actor(&actor_id, Subject::default())
            .await
            .unwrap();

        let response = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap();
        if let SubjectResponse::Metadata(metadata) = response {
            assert_eq!(metadata.namespace, Namespace::from("namespace"));
        } else {
            panic!("Invalid response");
        }
    }

    #[test]
    fn test_serialize_deserialize() {
        let value = init_state("");
        let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let request = create_start_request_mock("issuer", node_keys.clone());
        let keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let event = KoreEvent::from_create_request(
            &keys,
            &request,
            0,
            &value,
            DigestDerivator::Blake3_256,
        )
        .unwrap();

        let ledger = Ledger::from(event);

        let signature =
            Signature::new(&ledger, &node_keys, DigestDerivator::Blake3_256)
                .unwrap();
        let signed_ledger = Signed {
            content: ledger,
            signature,
        };

        let subject_a =
            Subject::from_event(Some(keys), &signed_ledger).unwrap();

        let bytes = bincode::serialize(&subject_a).unwrap();

        let subject_b = bincode::deserialize::<Subject>(&bytes).unwrap();
        assert_eq!(subject_a.subject_id, subject_b.subject_id);
    }

    #[tokio::test]
    async fn test_get_events() {
        let system = create_system().await;
        let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());

        let (subject_actor, _ledger_event_actor, _subject, _signed_ledger) =
            create_subject_and_ledger_event(system, node_keys.clone()).await;

        let response = subject_actor
            .ask(SubjectMessage::GetLedger { last_sn: 0 })
            .await
            .unwrap();
        if let SubjectResponse::Ledger(ledger) = response {
            assert!(ledger.0.len() == 1);
            assert!(ledger.1.is_some());
        } else {
            panic!("Invalid response");
        }

        let response = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap();

        if let SubjectResponse::Metadata(metadata) = response {
            assert_eq!(metadata.sn, 0);
        } else {
            panic!("Invalid response");
        }
    }

    #[tokio::test]
    async fn test_1000_events() {
        let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let system = create_system().await;

        let (subject_actor, _ledger_event_actor, subject, signed_ledger) =
            create_subject_and_ledger_event(system, node_keys.clone()).await;

        let hash_pre_event =
            signed_ledger.hash_id(DigestDerivator::Blake3_256).unwrap();

        let inicio = Instant::now();
        let response = subject_actor
            .ask(SubjectMessage::UpdateLedger {
                events: create_n_fact_events(
                    hash_pre_event,
                    1000,
                    node_keys,
                    subject.subject_id,
                    subject.properties.0,
                ),
            })
            .await
            .unwrap();
        let duracion = inicio.elapsed();
        println!("El método tardó: {:.2?}", duracion);

        if let SubjectResponse::LastSn(last_sn) = response {
            assert_eq!(last_sn, 1000);
        } else {
            panic!("Invalid response");
        }

        let response = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap();

        if let SubjectResponse::Metadata(metadata) = response {
            assert_eq!(metadata.sn, 1000);
        } else {
            panic!("Invalid response");
        }

        let response = subject_actor
            .ask(SubjectMessage::GetGovernance)
            .await
            .unwrap();

        if let SubjectResponse::Governance(gov) = response {
            assert_eq!(gov.version, 1000);
        } else {
            panic!("Invalid response");
        }
    }
}
