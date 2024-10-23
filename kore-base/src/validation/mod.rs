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
        network::TimeOutResponse,
        signature::Signed,
        Namespace, SignTypesNode, SignTypesSubject,
    },
    request::manager::{RequestManager, RequestManagerMessage},
    subject::{Metadata, Subject, SubjectMessage, SubjectResponse},
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
    pub metadata: Metadata,
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
                .ask(SubjectMessage::SignRequest(Box::new(
                    SignTypesSubject::Validation(proof.clone()),
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
            Err(_e) => return Err(_e),
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
            "who: ALL, error: No validator was able to validate the event."
                .clone_into(&mut error);
        }

        let req_path =
            ActorPath::from(format!("/user/request/{}", self.request_id));
        let req_actor: Option<ActorRef<RequestManager>> =
            ctx.system().get_actor(&req_path).await;

        if let Some(req_actor) = req_actor {
            if let Err(_e) = req_actor
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

    async fn try_to_update(&self, ctx: &mut ActorContext<Validation>) {
        let mut all_time_out = true;

        for response in self.validators_response.clone() {
            if let ProtocolsSignatures::Signature(_) = response {
                all_time_out = false;
                break;
            }
        }

        if all_time_out {
            todo!()
        }
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
        _sender: ActorPath,
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
                    SignTypesNode::ValidationReq(Box::new(
                        validation_req.clone(),
                    )),
                )
                .await
                {
                    Ok(signature) => signature,
                    Err(_e) => todo!(),
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

                        if let Err(_e) =
                            self.send_validation_to_req(ctx, true).await
                        {
                            todo!()
                        };
                    } else if self.validators.is_empty() {
                        // TODO
                        // self.try_to_update(ctx).await;

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

                        if let Err(_e) =
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
        if let Err(_e) = self.persist(&event, ctx).await {
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

#[cfg(test)]
mod tests {
    use core::panic;
    use identity::identifier::derive::digest::DigestDerivator;
    use serde_json::to_value;
    use std::time::Duration;

    use actor::{ActorPath, ActorRef};
    use identity::{
        identifier::{DigestIdentifier, KeyIdentifier},
        keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
    };

    use crate::{
        model::{event::{LedgerValue, ProtocolsSignatures}, Namespace, SignTypesNode},
        request::{
            RequestHandler, RequestHandlerMessage, RequestHandlerResponse,
        },
        subject::event::{
            LedgerEvent, LedgerEventMessage, LedgerEventResponse,
        },
        tests::create_system,
        CreateRequest, EOLRequest, EventRequest, Governance, HashId, Node,
        NodeMessage, NodeResponse, Signed, Subject, SubjectMessage,
        SubjectResponse, ValueWrapper,
    };

    async fn create_subject_gov() -> (
        ActorRef<Node>,
        ActorRef<RequestHandler>,
        ActorRef<Subject>,
        ActorRef<LedgerEvent>,
        DigestIdentifier,
    ) {
        let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let system = create_system().await;

        let node = Node::new(&node_keys).unwrap();
        let node_actor = system.create_root_actor("node", node).await.unwrap();

        let request = RequestHandler::new(node_keys.key_identifier());
        let request_actor =
            system.create_root_actor("request", request).await.unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        let create_req = EventRequest::Create(CreateRequest {
            governance_id: DigestIdentifier::default(),
            schema_id: "governance".to_owned(),
            namespace: Namespace::new(),
        });

        let response = node_actor
            .ask(NodeMessage::SignRequest(SignTypesNode::EventRequest(
                create_req.clone(),
            )))
            .await
            .unwrap();
        let NodeResponse::SignRequest(signature) = response else {
            panic!("Invalid Response")
        };

        let signed_event_req = Signed {
            content: create_req,
            signature,
        };

        let RequestHandlerResponse::Ok(_response) = request_actor
            .ask(RequestHandlerMessage::NewRequest {
                request: signed_event_req.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        let NodeResponse::Subjects(subjects) =
            node_actor.ask(NodeMessage::GetSubjects).await.unwrap()
        else {
            panic!("Invalid response")
        };

        let temporal_subj = subjects.temporal_subjects[0].clone();

        tokio::time::sleep(Duration::from_secs(1)).await;
        let NodeResponse::Subjects(subjects) =
            node_actor.ask(NodeMessage::GetSubjects).await.unwrap()
        else {
            panic!("Invalid response")
        };

        let owned_subj = subjects.owned_subjects[0].clone();

        assert_eq!(temporal_subj, owned_subj);

        let subject_actor: ActorRef<Subject> = system
            .get_actor(&ActorPath::from(format!(
                "/user/node/{}",
                temporal_subj
            )))
            .await
            .unwrap();

        let ledger_event_actor: ActorRef<LedgerEvent> = system
            .get_actor(&ActorPath::from(format!(
                "/user/node/{}/ledgerEvent",
                temporal_subj
            )))
            .await
            .unwrap();

        let LedgerEventResponse::LastEvent(last_event) = ledger_event_actor
            .ask(LedgerEventMessage::GetLastEvent)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        tokio::time::sleep(Duration::from_secs(1)).await;

        let SubjectResponse::Metadata(metadata) = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(last_event.content.subject_id.to_string(), owned_subj);
        assert_eq!(last_event.content.event_request, signed_event_req);
        assert_eq!(last_event.content.sn, 0);
        assert_eq!(last_event.content.gov_version, 0);
        assert_eq!(
            last_event.content.value,
            LedgerValue::Patch(ValueWrapper(serde_json::Value::String(
                "[]".to_owned(),
            ),))
        );
        assert_eq!(
            last_event.content.state_hash,
            metadata
                .properties
                .hash_id(DigestDerivator::Blake3_256)
                .unwrap()
        );
        assert!(last_event.content.eval_success.is_none());
        assert!(!last_event.content.appr_required);
        assert!(last_event.content.appr_success.is_none());
        assert!(last_event.content.vali_success);
        assert_eq!(
            last_event.content.hash_prev_event,
            DigestIdentifier::default()
        );
        assert!(last_event.content.evaluators.is_none());
        assert!(last_event.content.approvers.is_none(),);
        assert!(!last_event.content.validators.is_empty());

        assert_eq!(metadata.subject_id.to_string(), owned_subj);
        assert_eq!(metadata.governance_id.to_string(), "");
        assert_eq!(metadata.genesis_gov_version, 0);
        assert_ne!(metadata.subject_public_key, KeyIdentifier::default());
        assert_eq!(metadata.schema_id, "governance");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 0);
        assert_eq!(metadata.owner, node_keys.key_identifier());
        assert!(metadata.active);

        let gov = Governance::try_from(metadata.properties).unwrap();
        assert_eq!(gov.version, 0);
        assert!(!gov.members.is_empty());
        assert!(!gov.roles.is_empty());
        assert!(gov.schemas.is_empty());
        assert!(gov.subjects_id.is_empty());
        assert!(!gov.policies.is_empty());

        (
            node_actor,
            request_actor,
            subject_actor,
            ledger_event_actor,
            metadata.subject_id,
        )
    }

    #[tokio::test]
    async fn test_create_req() {
        let _ = create_subject_gov().await;
    }

    #[tokio::test]
    async fn test_eol_req() {
        let (
            node_actor,
            request_actor,
            subject_actor,
            ledger_event_actor,
            subject_id,
        ) = create_subject_gov().await;

        let eol_reques = EventRequest::EOL(EOLRequest { subject_id: subject_id.clone() });

        let response = node_actor
            .ask(NodeMessage::SignRequest(SignTypesNode::EventRequest(
                eol_reques.clone(),
            )))
            .await
            .unwrap();
        let NodeResponse::SignRequest(signature) = response else {
            panic!("Invalid Response")
        };

        let signed_event_req = Signed {
            content: eol_reques,
            signature,
        };

        let RequestHandlerResponse::Ok(_response) = request_actor
            .ask(RequestHandlerMessage::NewRequest {
                request: signed_event_req.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        tokio::time::sleep(Duration::from_secs(2)).await;

        let LedgerEventResponse::LastEvent(last_event) = ledger_event_actor
            .ask(LedgerEventMessage::GetLastEvent)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        tokio::time::sleep(Duration::from_secs(1)).await;

        let SubjectResponse::Metadata(metadata) = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(last_event.content.subject_id, subject_id);
        assert_eq!(last_event.content.event_request, signed_event_req);
        assert_eq!(last_event.content.sn, 1);
        assert_eq!(last_event.content.gov_version, 0);
        assert_eq!(
            last_event.content.value,
            LedgerValue::Patch(ValueWrapper(serde_json::Value::String(
                "[]".to_owned(),
            ),))
        );
        assert!(last_event.content.eval_success.is_none());
        assert!(!last_event.content.appr_required);
        assert!(last_event.content.appr_success.is_none());
        assert!(last_event.content.vali_success);
        assert!(last_event.content.evaluators.is_none());
        assert!(last_event.content.approvers.is_none(),);
        assert!(!last_event.content.validators.is_empty());

        assert_eq!(metadata.subject_id, subject_id);
        assert_eq!(metadata.governance_id.to_string(), "");
        assert_eq!(metadata.genesis_gov_version, 0);
        assert_ne!(metadata.subject_public_key, KeyIdentifier::default());
        assert_eq!(metadata.schema_id, "governance");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 1);
        assert!(!metadata.active);

        let gov = Governance::try_from(metadata.properties).unwrap();
        assert_eq!(gov.version, 1);
        assert!(!gov.members.is_empty());
        assert!(!gov.roles.is_empty());
        assert!(gov.schemas.is_empty());
        assert!(gov.subjects_id.is_empty());
        assert!(!gov.policies.is_empty());

        let RequestHandlerResponse::Error(_response) = request_actor
        .ask(RequestHandlerMessage::NewRequest {
            request: signed_event_req.clone(),
        })
        .await
        .unwrap()
    else {
        panic!("Invalid response")
    };
    }
}
