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
    governance::{model::Roles, Quorum},
    model::{
        common::get_sign,
        event::{ProofEvent, ProtocolsSignatures},
        signature::Signed,
        Namespace, SignTypesNode, SignTypesSubject,
    },
    request::manager::{RequestManager, RequestManagerMessage},
    subject::{Subject, SubjectMessage, SubjectMetadata, SubjectResponse},
    Error,
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};

use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use proof::ValidationProof;
use request::ValidationReq;
use response::ValidationRes;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::debug;
use validator::{Validator, ValidatorMessage};

use std::collections::HashSet;

/// A struct for passing validation information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationInfo {
    pub metadata: SubjectMetadata,
    pub event_proof: Signed<ProofEvent>,
}

// TODO HAy errores de la validacion que obligan a reiniciarla, ya que hay que actualizar la governanza u otra cosa,
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Validation {
    node_key: KeyIdentifier,
    // Quorum
    quorum: Quorum,
    // Validators
    validators: HashSet<KeyIdentifier>,
    // Actual responses
    validators_response: Vec<ProtocolsSignatures>,
    // Validators quantity
    validators_quantity: u32,

    actual_proof: ValidationProof,

    errors: String,

    valid_validation: bool,

    request_id: String,

    previous_proof: Option<ValidationProof>,
    prev_event_validation_response: Vec<ProtocolsSignatures>,
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
        let prev_evet_hash =
            if let Some(previous_proof) = self.previous_proof.clone() {
                previous_proof.event_hash
            } else {
                DigestIdentifier::default()
            };

        // Create proof from validation info
        let proof =
            ValidationProof::from_info(validation_info, prev_evet_hash)?;

        // Subject path.
        let subject_path = ctx.path().parent();

        // Subject actor.
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        // We obtain the actor subject
        let response = if let Some(subject_actor) = subject_actor {
            // We ask a subject
            let response = subject_actor
                .ask(SubjectMessage::SignRequest(SignTypesSubject::Validation(
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
                .tell(ValidatorMessage::LocalValidation {
                    validation_req: validation_req.content,
                    our_key: signer,
                })
                .await?
        }
        // Other node is signer
        else {
            validator_actor
                .tell(ValidatorMessage::NetworkValidation {
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

    async fn send_validation_to_req(
        &self,
        ctx: &mut ActorContext<Validation>,
        result: bool,
    ) -> Result<(), Error> {
        let mut error = self.errors.clone();
        if !result && error.is_empty() {
            error =
                "who: ALL, error: No validator was able to validate the event."
                    .to_owned()
        }

        let req_path =
            ActorPath::from(format!("/user/request/{}", self.request_id));
        let req_actor: Option<ActorRef<RequestManager>> =
            ctx.system().get_actor(&req_path).await;

        if let Some(req_actor) = req_actor {
            if let Err(e) = req_actor
                .tell(RequestManagerMessage::ValidationRes {
                    result,
                    signatures: self.validators_response.clone(),
                    errors: error,
                })
                .await
            {
                todo!()
            }
        } else {
            todo!()
        };

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ValidationMessage {
    Create {
        request_id: String,
        info: ValidationInfo,
    },

    Response {
        validation_res: ValidationRes,
        sender: KeyIdentifier,
    },
}

impl Message for ValidationMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationEvent {
    pub actual_proof: ValidationProof,
    pub actual_event_validation_response: Vec<ProtocolsSignatures>,
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
    type Message = ValidationMessage;
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
        msg: ValidationMessage,
        ctx: &mut ActorContext<Validation>,
    ) -> Result<ValidationResponse, ActorError> {
        match msg {
            ValidationMessage::Create { request_id, info } => {
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
                        info.metadata.subject_id.clone(),
                        &info.metadata.schema_id,
                        info.metadata.namespace,
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
                self.valid_validation = false;
                self.errors = String::default();
                self.validators_response = vec![];
                self.quorum = quorum;
                self.validators.clone_from(&signers);
                self.validators_quantity = signers.len() as u32;
                self.request_id = request_id.to_string();

                let signature = match get_sign(
                    ctx,
                    SignTypesNode::ValidationReq(validation_req.clone()),
                )
                .await
                {
                    Ok(signature) => signature,
                    Err(e) => todo!(),
                };

                let signed_validation_req: Signed<ValidationReq> = Signed {
                    content: validation_req,
                    signature,
                };

                for signer in signers {
                    self.create_validators(
                        ctx,
                        &self.request_id,
                        signed_validation_req.clone(),
                        &info.metadata.schema_id,
                        signer,
                    )
                    .await?
                }
            }
            ValidationMessage::Response {
                validation_res,
                sender,
            } => {
                // TODO Al menos una validación tiene que ser válida, no solo errores y timeout.

                // If node is in validator list
                if self.check_validator(sender.clone()) {
                    match validation_res {
                        ValidationRes::Signature(signature) => {
                            self.valid_validation = true;
                            self.validators_response
                                .push(ProtocolsSignatures::Signature(signature))
                        }
                        ValidationRes::TimeOut(timeout) => self
                            .validators_response
                            .push(ProtocolsSignatures::TimeOut(timeout)),
                        ValidationRes::Error(error) => {
                            self.errors = format!(
                                "{} who: {}, error: {}.",
                                self.errors, sender, error
                            );
                        }
                    };

                    if self.quorum.check_quorum(
                        self.validators_quantity,
                        self.validators_response.len() as u32,
                    ) && self.valid_validation
                    {
                        // The quorum was met, we persisted, and we applied the status
                        self.on_event(
                            ValidationEvent {
                                actual_proof: self.actual_proof.clone(),
                                actual_event_validation_response: self
                                    .validators_response
                                    .clone(),
                            },
                            ctx,
                        )
                        .await;

                        if let Err(e) =
                            self.send_validation_to_req(ctx, true).await
                        {
                            todo!()
                        };
                    } else if self.validators.is_empty() {
                        // we have received all the responses and the quorum has not been met
                        self.on_event(
                            ValidationEvent {
                                actual_proof: self.actual_proof.clone(),
                                actual_event_validation_response: self
                                    .validators_response
                                    .clone(),
                            },
                            ctx,
                        )
                        .await;

                        if let Err(e) =
                            self.send_validation_to_req(ctx, false).await
                        {
                            todo!()
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
