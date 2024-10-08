// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Validation module.
//!

pub mod proof;
pub mod request;
pub mod response;
pub mod schema;
pub mod validator;

use crate::{
    db::Storable,
    governance::{model::Roles, Quorum, RequestStage},
    model::{
        event::{Event as KoreEvent, ProofEvent},
        signature::Signed,
        Namespace, SignTypesNode, SignTypesSubject,
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
    pub event: Signed<ProofEvent>,
    pub prev_proof_event_hash: DigestIdentifier,
    pub gov_version: u64,
    pub owner: KeyIdentifier,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Validation {
    node_key: KeyIdentifier,
    // Quorum
    quorum: Quorum,
    // Validators
    validators: HashSet<KeyIdentifier>,
    // Actual responses
    validators_response: Vec<SignersRes>,
    // Validators quantity
    validators_quantity: u32,

    actual_proof: ValidationProof,

    previous_proof: Option<ValidationProof>,
    prev_event_validation_response: Vec<SignersRes>,
}

impl Validation {
    pub fn new(node_key: KeyIdentifier) -> Self {
        Validation {
            node_key,
            ..Default::default()
        }
    }

    fn check_validator(&mut self, validator: KeyIdentifier) -> bool {
        self.validators.remove(&validator)
    }

    async fn create_validation_req(
        &self,
        ctx: &mut ActorContext<Validation>,
        validation_info: ValidationInfo,
    ) -> Result<(ValidationReq, ValidationProof), Error> {
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
                .ask(SubjectCommand::SignRequest(SignTypesSubject::Validation(
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
                return Err(Error::Actor("An unexpected response has been received from subject actor".to_owned()));
            }
        };

        Ok((
            ValidationReq {
                proof: proof.clone(),
                subject_signature,
                previous_proof: self.previous_proof.clone(),
                prev_event_validation_response: self
                    .prev_event_validation_response
                    .clone(),
            },
            proof,
        ))
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
        let governance_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&governance_path).await;

        // We obtain the actor governance
        let response = if let Some(governance_actor) = governance_actor {
            // We ask a governance
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
                governance
            )));
        };

        // We handle the possible responses of governance
        match response {
            SubjectResponse::Governance(gov) => {
                match gov.get_quorum_and_signers(Roles::VALIDATOR, schema_id, namespace) {
                    Ok(quorum_and_signers) => Ok(quorum_and_signers),
                    Err(error) => Err(Error::Actor(format!("The governance encountered problems when getting signers and quorum: {}",error)))
                }
            }
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

    async fn create_validators(
        &self,
        ctx: &mut ActorContext<Validation>,
        request_id: &str,
        validation_req: Signed<ValidationReq>,
        schema: &str,
        signer: KeyIdentifier,
    ) -> Result<(), ActorError> {
        // Create Validator child
        let child = ctx
            .create_child(
                &format!("{}", signer),
                Validator::new(request_id.to_owned(), signer.clone()),
            )
            .await;
        let validator_actor = match child {
            Ok(child) => child,
            Err(e) => return Err(e),
        };

        // Check node_key
        let our_key = self.node_key.clone();
        // We are signer
        if signer == our_key {
            validator_actor
                .tell(ValidatorCommand::LocalValidation {
                    validation_req: validation_req.content,
                    our_key: signer,
                })
                .await?
        }
        // Other node is signer
        else {
            validator_actor
                .tell(ValidatorCommand::NetworkValidation {
                    request_id: request_id.to_owned(),
                    validation_req,
                    node_key: signer,
                    our_key,
                    schema: schema.to_owned(),
                })
                .await?
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

    Response {
        validation_res: ValidationRes,
        sender: KeyIdentifier,
    },
}

impl Message for ValidationCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationEvent {
    pub actual_proof: ValidationProof,
    pub actual_event_validation_response: Vec<SignersRes>,
    pub validation: bool,
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
        let prefix = ctx.path().parent().key();
        self.init_store("validation", Some(prefix), false, ctx)
            .await
    }

    async fn pre_stop(
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
        sender: ActorPath,
        msg: ValidationCommand,
        ctx: &mut ActorContext<Validation>,
    ) -> Result<ValidationResponse, ActorError> {
        match msg {
            ValidationCommand::Create { request_id, info } => {
                let (validation_req, proof) =
                    match self.create_validation_req(ctx, info.clone()).await {
                        Ok(validation_req) => validation_req,
                        Err(e) => {
                            // Mensaje al padre de error return Ok(ValidationResponse::Error(e))
                            return Ok(ValidationResponse::Error(e));
                        }
                    };
                self.actual_proof = proof;

                // Get signers and quorum
                let (signers, quorum) = match self
                    .get_signers_and_quorum(
                        ctx,
                        info.subject.subject_id.clone(),
                        &info.subject.schema_id,
                        info.subject.namespace,
                    )
                    .await
                {
                    Ok(signers_quorum) => signers_quorum,
                    Err(e) => {
                        // Mensaje al padre de error return Ok(ValidationResponse::Error(e))
                        return Ok(ValidationResponse::Error(e));
                    }
                };

                // Update quorum and validators
                self.validators_response = vec![];
                self.quorum = quorum;
                self.validators.clone_from(&signers);
                self.validators_quantity = signers.len() as u32;
                let request_id = request_id.to_string();

                let node_path = ActorPath::from("/user/node");
                let node_actor: Option<ActorRef<Node>> =
                    ctx.system().get_actor(&node_path).await;

                // We obtain the validator
                let node_response = if let Some(node_actor) = node_actor {
                    match node_actor
                        .ask(NodeMessage::SignRequest(
                            SignTypesNode::ValidationReq(
                                validation_req.clone(),
                            ),
                        ))
                        .await
                    {
                        Ok(response) => response,
                        Err(e) => todo!(),
                    }
                } else {
                    todo!()
                };

                let signature = match node_response {
                    NodeResponse::SignRequest(signature) => signature,
                    NodeResponse::Error(_) => todo!(),
                    _ => todo!(),
                };

                let signed_validation_req: Signed<ValidationReq> = Signed {
                    content: validation_req,
                    signature,
                };

                for signer in signers {
                    self.create_validators(
                        ctx,
                        &request_id,
                        signed_validation_req.clone(),
                        &info.subject.schema_id,
                        signer,
                    )
                    .await?
                }
            }
            ValidationCommand::Response {
                validation_res,
                sender,
            } => {
                // TODO Al menos una validación tiene que ser válida, no solo errores y timeout.

                // If node is in validator list
                if self.check_validator(sender) {
                    match validation_res {
                        ValidationRes::Signature(signature) => self
                            .validators_response
                            .push(SignersRes::Signature(signature)),
                        ValidationRes::TimeOut(timeout) => self
                            .validators_response
                            .push(SignersRes::TimeOut(timeout)),
                        ValidationRes::Error(error) => {
                            // Mostrar el error TODO
                        }
                    };

                    if self.quorum.check_quorum(
                        self.validators_quantity,
                        self.validators_response.len() as u32,
                    ) {
                        // The quorum was met, we persisted, and we applied the status
                        if let Err(e) = ctx
                            .event(ValidationEvent {
                                actual_proof: self.actual_proof.clone(),
                                actual_event_validation_response: self
                                    .validators_response
                                    .clone(),
                                validation: true,
                            })
                            .await
                        {
                            // TODO error al persistir, propagar hacia arriba
                        };
                    } else if self.validators.is_empty() {
                        // we have received all the responses and the quorum has not been met
                        if let Err(e) = ctx
                            .event(ValidationEvent {
                                actual_proof: self.actual_proof.clone(),
                                actual_event_validation_response: self
                                    .validators_response
                                    .clone(),
                                validation: false,
                            })
                            .await
                        {
                            // TODO error al persistir, propagar hacia arriba
                        };
                    }
                } else {
                    // TODO la respuesta no es válida, nos ha llegado una validación de alguien que no esperabamos o ya habíamos recibido la respuesta.
                }
            }
        };
        Ok(ValidationResponse::None)
    }
    async fn on_event(
        &mut self,
        event: ValidationEvent,
        ctx: &mut ActorContext<Validation>,
    ) {
        if let Err(e) = self.persist(&event, ctx).await {
            // TODO error al persistir, propagar hacia arriba
        };
    }
}

#[async_trait]
impl PersistentActor for Validation {
    fn apply(&mut self, event: &ValidationEvent) {
        self.prev_event_validation_response
            .clone_from(&event.actual_event_validation_response);
        self.previous_proof = Some(event.actual_proof.clone());

        // Darle a request la conclusión de la validación y la información que necesite. Esto no se puede hacer en el apply TODO
    }
}

impl Storable for Validation {}
