// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Evaluation module.
//! This module contains the evaluation logic for the Kore protocol.
//!

pub mod compiler;
pub mod evaluator;
pub mod request;
pub mod response;
mod runner;
pub mod schema;

use crate::{
    db::Storable,
    governance::{model::Roles, Governance, Quorum, RequestStage},
    model::{
        common::{get_metadata, get_sign},
        event::{
            Event as KoreEvent, LedgerValue, ProtocolsError,
            ProtocolsSignatures,
        },
        namespace,
        request::EventRequest,
        signature::{self, Signature, Signed},
        HashId, Namespace, SignTypesNode,
    },
    node::{Node, NodeMessage, NodeResponse},
    request::manager::{RequestManager, RequestManagerMessage},
    subject::{Subject, SubjectMessage, SubjectMetadata, SubjectResponse},
    Error, ValueWrapper, DIGEST_DERIVATOR,
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use evaluator::{Evaluator, EvaluatorMessage};
use identity::identifier::{
    derive::digest::DigestDerivator, key_identifier, DigestIdentifier,
    KeyIdentifier,
};
use request::{EvaluationReq, SubjectContext};
use response::{EvalLedgerResponse, EvaluationRes, Response as EvalRes};
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};
use wasmtime::Engine;

use std::{collections::HashSet, fs::Metadata, time::Duration};
// TODO cuando se recibe una evaluación, validación lo que sea debería venir firmado y comprobar que es de quien dice ser, cuando llega por la network y cuando la envía un usuario.
#[derive(Default)]
pub struct Evaluation {
    node_key: KeyIdentifier,
    // Quorum
    quorum: Quorum,
    // Evaluators
    evaluators: HashSet<KeyIdentifier>,
    // Actual responses
    evaluators_response: Vec<EvalRes>,
    // Evaluators quantity
    evaluators_quantity: u32,

    evaluators_signatures: Vec<ProtocolsSignatures>,
    request_id: String,
    errors: String,

    eval_req: Option<EvaluationReq>,
}

impl Evaluation {
    pub fn new(node_key: KeyIdentifier) -> Self {
        Evaluation {
            node_key,
            ..Default::default()
        }
    }

    fn check_evaluator(&mut self, evaluator: KeyIdentifier) -> bool {
        self.evaluators.remove(&evaluator)
    }

    fn create_evaluation_req(
        &self,
        event_request: Signed<EventRequest>,
        metadata: SubjectMetadata,
        gov_version: u64,
    ) -> EvaluationReq {
        EvaluationReq {
            event_request: event_request.clone(),
            context: SubjectContext {
                subject_id: metadata.subject_id,
                governance_id: metadata.governance_id,
                schema_id: metadata.schema_id,
                is_owner: self.node_key == event_request.signature.signer,
                state: metadata.properties,
                namespace: metadata.namespace.to_string(),
            },
            sn: metadata.sn + 1,
            gov_version,
        }
    }

    async fn get_signers_and_quorum_and_gov_version(
        &self,
        ctx: &mut ActorContext<Evaluation>,
        governance: DigestIdentifier,
        schema_id: &str,
        namespace: Namespace,
    ) -> Result<(HashSet<KeyIdentifier>, Quorum, u64), Error> {
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
                match gov.get_quorum_and_signers(Roles::EVALUATOR, schema_id, namespace) {
                    Ok((signers, quorum)) => Ok((signers, quorum, gov.version)),
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

    async fn create_evaluators(
        &self,
        ctx: &mut ActorContext<Evaluation>,
        request_id: &str,
        evaluation_req: Signed<EvaluationReq>,
        schema: &str,
        signer: KeyIdentifier,
    ) -> Result<(), ActorError> {
        // Create Evaluator child
        let child = ctx
            .create_child(&format!("{}", signer), Evaluator::default())
            .await;
        let evaluator_actor = match child {
            Ok(child) => child,
            Err(e) => return Err(e),
        };

        // Check node_key
        let our_key = self.node_key.clone();
        // We are signer
        if signer == our_key {
            evaluator_actor
                .tell(EvaluatorMessage::LocalEvaluation {
                    evaluation_req: evaluation_req.content,
                    our_key: signer,
                })
                .await?
        }
        // Other node is signer
        else {
            evaluator_actor
                .tell(EvaluatorMessage::NetworkEvaluation {
                    request_id: request_id.to_owned(),
                    evaluation_req,
                    node_key: signer,
                    our_key,
                    schema: schema.to_owned(),
                })
                .await?
        }

        Ok(())
    }

    fn check_responses(&self) -> bool {
        let set: HashSet<EvalRes> =
            HashSet::from_iter(self.evaluators_response.iter().cloned());

        set.len() == 1
    }

    fn fail_evaluation(&self) -> EvalLedgerResponse {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };
        let state = if let Some(req) = self.eval_req.clone() {
            req.context.state
        } else {
            todo!()
        };

        let state_hash = match state.hash_id(derivator) {
            Ok(state_hash) => state_hash,
            Err(e) => todo!(),
        };

        let mut error = self.errors.clone();
        if self.errors.is_empty() {
            error =
                "who: ALL, error: No evaluator was able to evaluate the event."
                    .to_owned()
        }

        EvalLedgerResponse {
            value: LedgerValue::Error(ProtocolsError {
                evaluation: Some(error),
                validation: None,
            }),
            state_hash,
            eval_success: false,
            appr_required: false,
        }
    }

    async fn send_evaluation_to_req(
        &self,
        ctx: &mut ActorContext<Evaluation>,
        response: EvalLedgerResponse,
    ) -> Result<(), Error> {
        let req_path =
            ActorPath::from(format!("/user/request/{}", self.request_id));
        let req_actor: Option<ActorRef<RequestManager>> =
            ctx.system().get_actor(&req_path).await;

        let request = if let Some(req) = self.eval_req.clone() {
            req
        } else {
            todo!()
        };

        if let Some(req_actor) = req_actor {
            if let Err(e) = req_actor
                .tell(RequestManagerMessage::EvaluationRes {
                    request,
                    response,
                    signatures: self.evaluators_signatures.clone(),
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
pub enum EvaluationMessage {
    Create {
        request_id: String,
        request: Signed<EventRequest>,
    },

    Response {
        evaluation_res: EvaluationRes,
        sender: KeyIdentifier,
        signature: Option<Signature>,
    },
}

impl Message for EvaluationMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationEvent {}

impl Event for EvaluationEvent {}

#[derive(Debug, Clone)]
pub enum EvaluationResponse {
    Error(Error),
    None,
}

impl Response for EvaluationResponse {}

#[async_trait]
impl Actor for Evaluation {
    type Event = EvaluationEvent;
    type Message = EvaluationMessage;
    type Response = EvaluationResponse;
}

// TODO: revizar todos los errores, algunos pueden ser ActorError.
#[async_trait]
impl Handler<Evaluation> for Evaluation {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: EvaluationMessage,
        ctx: &mut ActorContext<Evaluation>,
    ) -> Result<EvaluationResponse, ActorError> {
        match msg {
            EvaluationMessage::Create {
                request_id,
                request,
            } => {
                let subject_id = if let EventRequest::Fact(event) =
                    request.content.clone()
                {
                    event.subject_id
                } else {
                    // Error evento incorrecto
                    todo!()
                };

                let metadata =
                    match get_metadata(ctx, &subject_id.to_string()).await {
                        Ok(metadata) => metadata,
                        Err(e) => {
                            // No se puede obtener la metadata
                            todo!()
                        }
                    };

                let governance = if metadata.governance_id.digest.is_empty() {
                    metadata.subject_id.clone()
                } else {
                    metadata.governance_id.clone()
                };

                let (signers, quorum, gov_version) = match self
                    .get_signers_and_quorum_and_gov_version(
                        ctx,
                        governance,
                        &metadata.schema_id,
                        metadata.namespace.clone(),
                    )
                    .await
                {
                    Ok(data) => data,
                    Err(e) => {
                        // No se puede obtener signers, quorum y gov_ver
                        todo!()
                    }
                };

                let eval_req = self.create_evaluation_req(
                    request,
                    metadata.clone(),
                    gov_version,
                );

                self.evaluators_response = vec![];
                self.eval_req = Some(eval_req.clone());
                self.quorum = quorum;
                self.evaluators.clone_from(&signers);
                self.evaluators_quantity = signers.len() as u32;
                self.request_id = request_id.to_string();
                self.evaluators_signatures = vec![];
                self.errors = String::default();

                let signature = match get_sign(
                    ctx,
                    SignTypesNode::EvaluationReq(eval_req.clone()),
                )
                .await
                {
                    Ok(signature) => signature,
                    Err(e) => todo!(),
                };

                let signed_evaluation_req: Signed<EvaluationReq> = Signed {
                    content: eval_req,
                    signature,
                };

                for signer in signers {
                    self.create_evaluators(
                        ctx,
                        &self.request_id,
                        signed_evaluation_req.clone(),
                        &metadata.schema_id,
                        signer,
                    )
                    .await?
                }
            }
            EvaluationMessage::Response {
                evaluation_res,
                sender,
                signature,
            } => {
                // TODO Al menos una validación tiene que ser válida, no solo errores y timeout.

                // If node is in evaluator list
                if self.check_evaluator(sender.clone()) {
                    // Check type of validation
                    match evaluation_res {
                        EvaluationRes::Response(response) => {
                            if let Some(signature) = signature {
                                self.evaluators_signatures.push(
                                    ProtocolsSignatures::Signature(signature),
                                );
                            } else {
                                todo!()
                            }
                            self.evaluators_response.push(response);
                        }
                        EvaluationRes::TimeOut(timeout) => self
                            .evaluators_signatures
                            .push(ProtocolsSignatures::TimeOut(timeout)),
                        EvaluationRes::Error(error) => {
                            self.errors = format!(
                                "{} who: {}, error: {}.",
                                self.errors, sender, error
                            );
                        }
                    };

                    // Add validate response
                    // self.evaluators_response.push(validate);
                    if self.quorum.check_quorum(
                        self.evaluators_quantity,
                        self.evaluators_response.len() as u32,
                    ) {
                        let response = if self.check_responses() {
                            EvalLedgerResponse::from(
                                self.evaluators_response[0].clone(),
                            )
                        } else {
                            self.fail_evaluation()
                        };

                        if let Err(e) =
                            self.send_evaluation_to_req(ctx, response).await
                        {
                        };
                    } else {
                        if self.evaluators.is_empty() {
                            let response = self.fail_evaluation();
                            if let Err(e) =
                                self.send_evaluation_to_req(ctx, response).await
                            {
                            };
                        }
                    }
                } else {
                    // TODO la respuesta no es válida, nos ha llegado una validación de alguien que no esperabamos o ya habíamos recibido la respuesta.
                }
            }
        }

        Ok(EvaluationResponse::None)
    }
}
