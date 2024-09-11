// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use actor::{
    Actor, ActorContext, Error as ActorError, Handler, Message, Response,
};
use actor::{ActorPath, ActorRef, Event};
use approver::{Approver, ApproverCommand};
use async_trait::async_trait;
use identity::identifier::derive::digest::DigestDerivator;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use memsecurity::Blake3Hash;
use request::ApprovalRequest;
use response::{ApprovalEntity, ApprovalResponse};
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};

use crate::evaluation::response::EvaluationRes;
use crate::governance::RequestStage;
use crate::model::{namespace, Namespace, SignTypesNode};
use crate::validation::response::ValidationRes;
use crate::{
    db::Storable,
    evaluation::{
        self, request::EvaluationReq, response::Response as EvalRes,
        EvaluationResponse,
    },
    governance::{model::Validation, Quorum},
    validation::request::ValidationReq,
    Error, Governance, Signature, Signed, Subject,
};
use crate::{
    governance, Node, NodeMessage, NodeResponse, SubjectCommand,
    SubjectResponse, DIGEST_DERIVATOR,
};

pub mod approver;
pub mod request;
pub mod response;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Approval {
    node_key: KeyIdentifier,
    // Quorum
    quorum: Quorum,
    // approvers
    approvers: HashSet<KeyIdentifier>,
    // Actual responses
    approvers_response: Vec<(Signature, bool)>,
    // approvers quantity
    approvers_quantity: u32,
}

impl Approval {
    pub fn new(node_key: KeyIdentifier) -> Self {
        Approval {
            node_key,
            ..Default::default()
        }
    }

    // generate the approval request
    async fn create_approval_req(
        &mut self,
        request_id: DigestIdentifier,
        ctx: &mut ActorContext<Approval>,
        req_evaluation: EvaluationReq,
        res_evaluation: EvalRes,
    ) -> Result<ApprovalRequest, Error> {
        // Get the derivator of the node
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            derivator.clone()
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };
        // Obtain the last event of subject actor
        let subject_path = ctx.path().parent();
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;
        let subject_actor = match subject_actor {
            Some(actor) => actor,
            None => {
                return Err(Error::Actor(format!(
                    "The subject actor was not found in the expected path {}",
                    subject_path
                )));
            }
        };
        let response = subject_actor.ask(SubjectCommand::LastEvent).await;
        let prev_event = match response {
            Ok(SubjectResponse::LastEvent(event)) => event,
            _ => todo!(),
        };
        // Hash of the previous event to add to the approval request
        let hash_prev_event = match DigestIdentifier::from_serializable_borsh(
            prev_event, derivator,
        ) {
            Ok(hash) => hash,
            Err(_) => todo!(),
        };
        // Obtengo el governance_id, si es vacio??
        let response = subject_actor.ask(SubjectCommand::GetSubjectState).await;
        let governance_id = match response {
            Ok(SubjectResponse::SubjectState(governance)) => governance.governance_id,
            _ => {
                return Err(Error::Actor(format!("")));
            }
        };
        Ok(ApprovalRequest {
            event_request: req_evaluation.event_request,
            sn: req_evaluation.sn,
            gov_version: req_evaluation.gov_version,
            patch: res_evaluation.patch,
            state_hash: res_evaluation.state_hash,
            hash_prev_event,
            governance_id,
        })
    }
    /// TODO refacorizar ya que solo varia la validation stage(se usa en todos los protocolos)
    async fn get_signers_and_quorum(
        &self,
        ctx: &mut ActorContext<Approval>,
        governance: &DigestIdentifier,
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
                match gov.get_quorum_and_signers(RequestStage::Approve, schema_id, namespace) {
                    Ok(quorum_and_signers) => Ok(quorum_and_signers),
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

    async fn create_approvers(
        &self,
        ctx: &mut ActorContext<Approval>,
        request_id: &str,
        approval_req: Signed<ApprovalRequest>,
        signer: KeyIdentifier,
    ) -> Result<(), ActorError> {
        // Create Approvers child
        let child = ctx
            .create_child(
                &format!("{}", signer),
                Approver::new(request_id.to_owned(), signer.clone()),
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
            if let Err(e) = validator_actor
                .tell(ApproverCommand::LocalApprover {
                    approval_req: approval_req,
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
                .tell(ApproverCommand::NetworkApprover {
                    request_id: request_id.to_owned(),
                    approval_req: approval_req.clone(),
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
    fn check_approval(&mut self, approver: KeyIdentifier) -> bool {
        self.approvers.remove(&approver)
    }
}

#[derive(Debug, Clone)]
pub enum ApprovalCommand {
    Create {
        request_id: DigestIdentifier,
        info: EvaluationReq,
        response: EvalRes,
    },
    Response {
        approval_res: ApprovalResponse,
        sender: KeyIdentifier,
    },
}

impl Message for ApprovalCommand {}


// 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalEvent {
    pub actual_event_approval_response: Vec<(Signature, bool)>,
    pub approval: bool,
}

impl Event for ApprovalEvent {}

#[derive(Debug, Clone)]
pub enum ApprovalRes {
    Error(Error),
    None,
}
impl Response for ApprovalRes {}

#[async_trait]
impl Actor for Approval {
    type Event = ApprovalEvent;
    type Message = ApprovalCommand;
    type Response = ApprovalRes;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Starting approval actor with init store.");
        self.init_store("approval", false, ctx).await
        // Una vez recuperado el estado debemos ver si el propio nodo ha recibido ya ha enviado la respuesta
        //para no levantar un approver
    }
    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Stopping approval actor with stop store.");
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<Approval> for Approval {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: ApprovalCommand,
        ctx: &mut ActorContext<Self>,
    ) -> Result<ApprovalRes, ActorError> {
        match msg {
            ApprovalCommand::Create {
                request_id,
                info,
                response,
            } => {
                // Creamos una petición de aprobación, miramos quorum y lanzamos approvers
                let approval_req = match self
                    .create_approval_req(
                        request_id.clone(),
                        ctx,
                        info.clone(),
                        response,
                    )
                    .await
                {
                    Ok(approval_req) => approval_req,
                    Err(e) => return Ok(ApprovalRes::Error(e)),
                };
                // Get signers and quorum
                let (signers, quorum) = match self
                    .get_signers_and_quorum(
                        ctx,
                        &info.context.subject_id,
                        &info.context.schema_id,
                        Namespace::from(info.context.namespace),
                    )
                    .await
                {
                    Ok(signers_quorum) => signers_quorum,
                    Err(e) => {
                        // Mensaje al padre de error return Ok(ValidationResponse::Error(e))
                        return Ok(ApprovalRes::Error(e));
                    }
                };
                // Update quorum and validators
                self.quorum = quorum;
                self.approvers = signers.clone();
                self.approvers_quantity = signers.len() as u32;
                let request_id = request_id.to_string();

                let node_path = ActorPath::from("/user/node");
                let node_actor: Option<ActorRef<Node>> =
                    ctx.system().get_actor(&node_path).await;

                // We obtain the validator
                let node_response = if let Some(node_actor) = node_actor {
                    match node_actor
                        .ask(NodeMessage::SignRequest(
                            SignTypesNode::ApprovalReq(approval_req.clone()),
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

                let signed_approval_req: Signed<ApprovalRequest> = Signed {
                    content: approval_req,
                    signature,
                };

                for signer in signers {
                    if let Err(error) = self
                        .create_approvers(
                            ctx,
                            &request_id,
                            signed_approval_req.clone(),
                            signer,
                        )
                        .await
                    {
                        // Mensaje al padre de error return Err(error);
                        return Err(error);
                    }
                }
            }
            ApprovalCommand::Response{approval_res, sender} => {
                if self.check_approval(sender) {
                    let node_key = match approval_res {
                        ApprovalResponse::Signature(signature, acceptance) => {
                            self.approvers_response.push((signature.clone(), acceptance));
                            signature.signer
                        }
                        ApprovalResponse::TimeOut(time_out) => time_out.who,
                        ApprovalResponse::Error(error) => error.who,
                    };
                }
                // si hemos llegado al quorum y hay suficientes aprobaciones aprobamos...
                if self.quorum.check_quorum(
                    self.approvers_quantity,
                    self.approvers_response.len() as u32,
                ) {
                    // Ahora necesito contabilizar sin han aprobado la mayoría para aprobar o no
                    let approval = self.quorum.check_quorum(
                        self.approvers_quantity,
                        self.approvers_response.iter().filter(|(_, approved)| *approved).count() as u32,
                    );
                    // The quorum was met, we persisted, and we applied the status
                    if let Err(e) = ctx
                        .event(ApprovalEvent {
                            actual_event_approval_response: self
                                .approvers_response
                                .clone(),
                            approval
                        })
                        .await
                    {
                        // TODO error al persistir, propagar hacia arriba
                    };
                }
            }
        }
        Ok(ApprovalRes::None)
    }
}

#[async_trait]
impl PersistentActor for Approval {
    fn apply(&mut self, event: &ApprovalEvent) {
        self.approvers_response = event.actual_event_approval_response.clone();
    }
}

impl Storable for Approval {}
