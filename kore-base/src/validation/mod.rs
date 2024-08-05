// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Validation module.
//!

pub mod proof;
pub mod request;
pub mod response;
pub mod validator;

use crate::{
    db::Storable,
    governance::{
        Governance, GovernanceCommand, GovernanceResponse, Quorum, RequestStage,
    },
    model::{
        event::Event as KoreEvent,
        namespace,
        request::EventRequest,
        signature::{Signature, Signed},
        HashId, Namespace, SignTypes,
    },
    node::{Node, NodeMessage, NodeResponse},
    subject::{Subject, SubjectCommand, SubjectResponse, SubjectState},
    Error, DIGEST_DERIVATOR,
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::{
    derive::digest::DigestDerivator, key_identifier, DigestIdentifier,
    KeyIdentifier,
};
use jsonschema::ValidationError;
use proof::ValidationProof;
use request::{SignersRes, ValidationReq};
use response::ValidationRes;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};
use validator::{Validator, ValidatorCommand};

use std::{collections::HashSet, time::Duration};

/// A struct for passing validation information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationInfo {
    pub subject: SubjectState,
    pub event: Signed<KoreEvent>,
    pub gov_version: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Validation {
    node_key: KeyIdentifier,
    // Quorum
    quorum: Quorum,
    // A quien preguntar
    validators: HashSet<KeyIdentifier>,
    // Respuestas
    validators_response: Vec<SignersRes>,
    previous_proof: Option<ValidationProof>,
    prev_event_validation_response: Vec<SignersRes>,
    in_validation: bool,
}

impl Validation {
    fn set_node_key(&mut self, node_key: KeyIdentifier) {
        self.node_key = node_key;
    }

    fn set_quorum(&mut self, quorum: Quorum) {
        self.quorum = quorum;
    }

    fn set_validators(&mut self, validators: HashSet<KeyIdentifier>) {
        self.validators = validators;
    }

    fn check_validator(&mut self, validator: KeyIdentifier) -> bool {
        self.validators.remove(&validator)
    }
    
    fn add_validator_response(&mut self, validator_res: SignersRes) {
        self.validators_response.push(validator_res);
    }

    async fn create_validation_req(
        &self,
        ctx: &mut ActorContext<Validation>,
        validation_info: ValidationInfo,
    ) -> Result<ValidationReq, Error> {
        // Create proof from validation info
        let proof = ValidationProof::from_info(validation_info)?;

        // Subject path.
        let subject_path = ctx.path().parent();

        // Subject actor.
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        // We obtain the actor subject
        let response = if let Some(subject_actor) = subject_actor {
            // We ask a subject
            let response = subject_actor
                .ask(SubjectCommand::SignRequest(SignTypes::Validation(
                    proof.clone(),
                )))
                .await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a subject {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The subject actor was not found in the expected path {}",
                subject_path
            )));
        };

        // We handle the possible responses of subject
        let subject_signature = match response {
            SubjectResponse::SignRequest(sign) => sign,
            SubjectResponse::Error(error) => {
                return Err(Error::Actor(format!("The subject encountered problems when signing the proof: {}",error)));
            }
            _ => {
                return Err(Error::Actor(format!("An unexpected response has been received from subject actor")));
            }
        };

        Ok(ValidationReq {
            proof,
            subject_signature,
            previous_proof: self.previous_proof.clone(),
            prev_event_validation_response: self
                .prev_event_validation_response
                .clone(),
        })
    }

    async fn get_node_key(
        &self,
        ctx: &mut ActorContext<Validation>,
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

    async fn get_signers_and_quorum(
        &self,
        ctx: &mut ActorContext<Validation>,
        governance: DigestIdentifier,
        schema_id: &str,
        namespace: Namespace,
    ) -> Result<(HashSet<KeyIdentifier>, Quorum), Error> {
        // Governance path.
        let governance_path =
            ActorPath::from(format!("/user/node/{}", governance));
        // Governance actor.
        let governance_actor: Option<ActorRef<Governance>> =
            ctx.system().get_actor(&governance_path).await;

        // We obtain the actor governance
        let response = if let Some(governance_actor) = governance_actor {
            // We ask a governance
            let response = governance_actor
                .ask(GovernanceCommand::GetSignersAndQuorum {
                    stage: RequestStage::Validate,
                    schema_id: schema_id.to_owned(),
                    namespace,
                })
                .await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a governance {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The governance actor was not found in the expected path /user/node/{}",
                governance
            )));
        };

        // We handle the possible responses of governance
        match response {
            GovernanceResponse::SignersAndQuorum(response) => Ok(response),
            GovernanceResponse::Error(error) => {
                return Err(Error::Actor(format!("The governance encountered problems when getting signers and quorum: {}",error)));
            }
            _ => Err(Error::Actor(format!(
                "An unexpected response has been received from node actor"
            ))),
        }
    }

    async fn create_validators(
        &self,
        ctx: &mut ActorContext<Validation>,
        request_id: &str,
        validation_req: ValidationReq,
        signer: KeyIdentifier,
    ) -> Result<(), ActorError> {
        // Create Validator child
        let child = ctx
            .create_child(&format!("{}", signer), Validator::default())
            .await;
        let validator_actor = match child {
            Ok(child) => child,
            Err(e) => return Err(e),
        };

        // Check node_key
        let our_key = self.node_key.clone();
        // We are signer
        if signer == our_key {
            if let Err(e) = validator_actor
                .tell(ValidatorCommand::LocalValidation {
                    validation_req,
                    our_key: signer,
                })
                .await
            {
                return Err(e);
            }
        }
        // Other node is signer
        else {
            if let Err(e) = validator_actor
                .tell(ValidatorCommand::NetworkValidation {
                    request_id: request_id.to_owned(),
                    validation_req,
                    node_key: signer,
                    our_key,
                })
                .await
            {
                return Err(e);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ValidationCommand {
    Create {
        request_id: DigestIdentifier,
        info: ValidationInfo,
    },

    Response(ValidationRes),
}

impl Message for ValidationCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationEvent {
    request_id: DigestIdentifier,
    info: ValidationInfo,
}

impl Event for ValidationEvent {}

#[derive(Debug, Clone)]
pub enum ValidationResponse {
    Error(Error),
    None,
}

impl Response for ValidationResponse {}

#[async_trait]
impl Actor for Validation {
    type Event = ValidationEvent;
    type Message = ValidationCommand;
    type Response = ValidationResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Starting validation actor with init store.");

        let node_key = match self.get_node_key(ctx).await {
            Ok(key) => key,
            Err(e) => {
                error!("Can not start Validation Actor, a problem getting keys of node: {}", e);
                return Err(ActorError::Create);
            }
        };
        // Update node_key
        self.set_node_key(node_key.clone());

        self.init_store("validation", true, ctx).await
    }

    async fn post_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Stopping validation actor with stop store.");
        self.stop_store(ctx).await
    }
}

// TODO: revizar todos los errores, algunos pueden ser ActorError.
#[async_trait]
impl Handler<Validation> for Validation {
    async fn handle_message(
        &mut self,
        msg: ValidationCommand,
        ctx: &mut ActorContext<Validation>,
    ) -> Result<ValidationResponse, ActorError> {
        match msg {
            ValidationCommand::Create { request_id, info } => {
                // Create Validation Request
                let validation_req =
                    match self.create_validation_req(ctx, info.clone()).await {
                        Ok(validation_req) => validation_req,
                        Err(e) => return Ok(ValidationResponse::Error(e)),
                    };

                // Get signers and quorum
                let (signers, quorum) = match self
                    .get_signers_and_quorum(
                        ctx,
                        info.subject.governance_id,
                        &info.subject.schema_id,
                        info.subject.namespace,
                    )
                    .await
                {
                    Ok(signers_quorum) => signers_quorum,
                    Err(e) => return Ok(ValidationResponse::Error(e)),
                };

                // Update quorum and validators
                self.set_quorum(quorum);
                self.set_validators(signers.clone());
                let request_id = request_id.to_string();

                for signer in signers {
                    if let Err(error) = self
                        .create_validators(
                            ctx,
                            &request_id,
                            validation_req.clone(),
                            signer,
                        )
                        .await
                    {
                        return Err(error);
                    }
                }

                Ok(ValidationResponse::None)
            }
            ValidationCommand::Response(response) => {
                let node_key = match response {
                    ValidationRes::Signature(signature) => signature.signer,
                    ValidationRes::TimeOut(time_out) => time_out.who,
                    ValidationRes::Error(error) => error.who,
                };

                // If node is in validator list
                if self.check_validator(node_key) {
                    
                } else {
                    // TODO la respuesta no es válida, nos ha llegado una validaciónde alguien que no esperabamos o ya habíamos recibido la respuesta.
                }
                
                // Mirar qué hijo ha respondido
                // Eliminarlo de la lista de pendientes
                // hay que mirar que las validaciones sean todas iguales ¿?
                // Comprar quorum, si quorum >= respuesta al padre (request)

                Ok(ValidationResponse::None)
            }
        }
    }
    async fn on_event(
        &mut self,
        event: ValidationEvent,
        ctx: &mut ActorContext<Validation>,
    ) {
        loop {
            if !self.in_validation {
                break;
            } else {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        let info = event.info;
        let request_id = event.request_id;

        let validation_req =
            match self.create_validation_req(ctx, info.clone()).await {
                Ok(validation_req) => validation_req,
                Err(e) => {
                    // Mensaje al padre de error return Ok(ValidationResponse::Error(e))
                    return ;
                },
            };

        // Get signers and quorum
        let (signers, quorum) = match self
            .get_signers_and_quorum(
                ctx,
                info.subject.governance_id,
                &info.subject.schema_id,
                info.subject.namespace,
            )
            .await
        {
            Ok(signers_quorum) => signers_quorum,
            Err(e) => {
                // Mensaje al padre de error return Ok(ValidationResponse::Error(e))
                return ;
            }
        };

        // Update quorum and validators
        self.set_quorum(quorum);
        self.set_validators(signers.clone());
        let request_id = request_id.to_string();

        for signer in signers {
            if let Err(error) = self
                .create_validators(
                    ctx,
                    &request_id,
                    validation_req.clone(),
                    signer,
                )
                .await
            {
                // Mensaje al padre de error return Err(error);
                return ;
            }
        }
        self.in_validation = true;
    }
}

#[async_trait]
impl PersistentActor for Validation {
    fn apply(&mut self, event: &ValidationEvent) {}
}

impl Storable for Validation {}
