// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Evaluation module.
//! This module contains the evaluation logic for the Kore protocol.
//!

mod compiler;
pub mod evaluator;
pub mod request;
pub mod response;
mod runner;

use crate::{
    db::Storable,
    governance::{Governance, Quorum, RequestStage},
    model::{
        event::Event as KoreEvent,
        namespace,
        request::EventRequest,
        signature::{self, Signature, Signed},
        HashId, Namespace, SignTypesNode,
    },
    node::{Node, NodeMessage, NodeResponse},
    subject::{
        Subject, SubjectCommand, SubjectMetadata, SubjectResponse, SubjectState,
    },
    Error, DIGEST_DERIVATOR,
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use evaluator::{Evaluator, EvaluatorCommand};
use identity::identifier::{
    derive::digest::DigestDerivator, key_identifier, DigestIdentifier,
    KeyIdentifier,
};
use request::{EvaluationReq, SubjectContext};
use response::EvaluationRes;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};
use wasmtime::Engine;

use std::{collections::HashSet, fs::Metadata, time::Duration};
// TODO cuando se recibe una evaluación, validación lo que sea debería venir firmado y comprobar que es de quien dice ser, cuando llega por la network y cuando la envía un usuario.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Evaluation {
    node_key: KeyIdentifier,
    // Quorum
    quorum: Quorum,
    // Evaluators
    evaluators: HashSet<KeyIdentifier>,
    // Actual responses
    evaluators_response: Vec<EvaluationRes>,
    // Evaluators quantity
    evaluators_quantity: u32,
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

    async fn get_metadata(
        &self,
        ctx: &mut ActorContext<Evaluation>,
        subject_id: DigestIdentifier,
    ) -> Result<SubjectMetadata, Error> {
        let subject_path =
            ActorPath::from(format!("/user/node/{}", subject_id));
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        let response = if let Some(subject_actor) = subject_actor {
            // We ask a node
            let response =
                subject_actor.ask(SubjectCommand::GetSubjectMetadata).await;
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
                "The node actor was not found in the expected path {}",
                subject_path
            )));
        };

        match response {
            SubjectResponse::SubjectMetadata(metadata) => Ok(metadata),
            _ => Err(Error::Actor(format!(
                "An unexpected response has been received from subject actor"
            ))),
        }
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
                match gov.get_quorum_and_signers(RequestStage::Evaluate, schema_id, namespace) {
                    Ok((signers, quorum)) => Ok((signers, quorum, gov.get_version())),
                    Err(error) => Err(Error::Actor(format!("The governance encountered problems when getting signers and quorum: {}",error)))
                }
            }
            SubjectResponse::Error(error) => {
                return Err(Error::Actor(format!("The subject encountered problems when getting governance: {}",error)));
            }
            _ => Err(Error::Actor(format!(
                "An unexpected response has been received from node actor"
            ))),
        }
    }

    async fn create_evaluators(
        &self,
        ctx: &mut ActorContext<Evaluation>,
        request_id: &str,
        evaluation_req: Signed<EvaluationReq>,
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
            if let Err(e) = evaluator_actor
                .tell(EvaluatorCommand::LocalEvaluation {
                    evaluation_req: evaluation_req.content,
                    our_key: signer,
                })
                .await
            {
                return Err(e);
            }
        }
        // Other node is signer
        else {
            if let Err(e) = evaluator_actor
                .tell(EvaluatorCommand::NetworkEvaluation {
                    request_id: request_id.to_owned(),
                    evaluation_req,
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
pub enum EvaluationCommand {
    Create {
        request_id: DigestIdentifier,
        request: Signed<EventRequest>,
    },

    Response(EvaluationRes),
}

impl Message for EvaluationCommand {}

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
    type Message = EvaluationCommand;
    type Response = EvaluationResponse;
}

// TODO: revizar todos los errores, algunos pueden ser ActorError.
#[async_trait]
impl Handler<Evaluation> for Evaluation {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: EvaluationCommand,
        ctx: &mut ActorContext<Evaluation>,
    ) -> Result<EvaluationResponse, ActorError> {
        match msg {
            EvaluationCommand::Create {
                request_id,
                request,
            } => {
                let subject_id = if let EventRequest::Fact(event) = request.content.clone() {
                    event.subject_id
                } else {
                    // Error evento incorrecto
                    todo!()
                };

                let metadata = match self.get_metadata(ctx, subject_id).await {
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

                let (signers, quorum, gov_version) = match self.get_signers_and_quorum_and_gov_version(ctx, governance, &metadata.schema_id, metadata.namespace.clone()).await {
                    Ok(data) => data,
                    Err(e) => {
                        // No se puede obtener signers, quorum y gov_ver
                        todo!()
                    }
                };

                let eval_req = self.create_evaluation_req(request, metadata, gov_version);

                self.quorum = quorum;
                self.evaluators = signers.clone();
                self.evaluators_quantity = signers.len() as u32;
                let request_id = request_id.to_string();

                let node_path = ActorPath::from("/user/node");
                let node_actor: Option<ActorRef<Node>> =  ctx.system().get_actor(&node_path).await;

                // We obtain the validator
                let node_response = if let Some(node_actor) = node_actor {
                    match node_actor.ask(NodeMessage::SignRequest(SignTypesNode::EvaluationReq(eval_req.clone()))).await {
                        Ok(response) => response,
                        Err(e) => todo!()
                    }
                } else {
                    todo!()
                };

                let signature = match node_response {
                    NodeResponse::SignRequest(signature) => signature,
                    NodeResponse::Error(_) => todo!(),
                    _ => todo!()
                };

                let signed_evaluation_req: Signed<EvaluationReq> = Signed { content: eval_req, signature };

                for signer in signers {
                    if let Err(error) = self
                        .create_evaluators(
                            ctx,
                            &request_id,
                            signed_evaluation_req.clone(),
                            signer,
                        )
                        .await
                    {
                        // Mensaje al padre de error return Err(error);
                        return Err(error);
                    }
                }
            },
            EvaluationCommand::Response(eval_res) => {
                todo!()
            },
        }

        Ok(EvaluationResponse::None)
    }
    async fn on_event(
        &mut self,
        event: EvaluationEvent,
        ctx: &mut ActorContext<Evaluation>,
    ) {
    }
}
