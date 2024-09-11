use std::f64::consts::E;

use crate::{
    db::Storable, model::{network::RetryNetwork, SignTypesNode}, ActorMessage, Error, EventRequest, Governance, NetworkMessage, Node, NodeMessage, NodeResponse, Signature, Signed, Subject, SubjectCommand, SubjectResponse, DIGEST_DERIVATOR
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event, ExponentialBackoffStrategy, Handler, Message, Response, RetryActor, RetryMessage, Strategy
};
use async_trait::async_trait;
use identity::identifier::{
    derive::digest::DigestDerivator, Derivable, DigestIdentifier, KeyIdentifier,
};
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::error;

use super::{
    request::ApprovalRequest,
    response::{
        ApprovalEntity, ApprovalError, ApprovalResponse, ApprovalState,
        VotationType,
    },
    Approval, ApprovalCommand, ApprovalRes,
};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Approver {
    node: KeyIdentifier,
    request_id: String,
    pass_votation: VotationType,
    request: Option<ApprovalEntity>,
}

fn subject_id_by_request(
    request: &EventRequest,
    gov_version: &u64,
    derivator: DigestDerivator,
) -> Result<DigestIdentifier, Error> {
    let subject_id = match request {
        EventRequest::Fact(ref fact_request) => fact_request.subject_id.clone(),
        EventRequest::Create(ref create_request) => {
            DigestIdentifier::from_serializable_borsh(
                (
                    &create_request.namespace,
                    &create_request.schema_id,
                    &create_request.public_key.to_str(),
                    &create_request.governance_id.to_str(),
                    gov_version,
                    derivator,
                ),
                derivator,
            )
            .map_err(|_| Error::Approval("()".to_string()))?
        }
        _ => {
            return Err(Error::Approval(
                "Only events of type Create and Fact are allowed.".to_string(),
            ))
        }
    };
    Ok(subject_id)
}

impl Approver {
    pub fn new(request_id: String, node: KeyIdentifier) -> Self {
        Approver {
            node,
            ..Default::default()
        }
    }
    // Refactorizar el get_gov se usa en todos los procesos
    async fn get_gov(
        &self,
        ctx: &mut ActorContext<Approver>,
        governance_id: DigestIdentifier,
    ) -> Result<Governance, Error> {
        // Governance path
        let governance_path =
            ActorPath::from(format!("/user/node/{}", governance_id));

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
                "The governance actor was not found in the expected path {}",
                governance_path
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

    async fn approval(
        &mut self,
        ctx: &mut ActorContext<Approver>,
        approval_request: Signed<ApprovalRequest>,
    ) -> Result<(), Error> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            derivator.clone()
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };
        // Obtengo el subject id de la request(FACT, CREATE(la genero))
        //let subject_id = subject_id_by_request(&approval_request.event_request.content,&approval_request.gov_version, derivator)?;

        let schema_id = match approval_request.content.event_request.content {
            EventRequest::Create(ref create_request) => {
                create_request.schema_id.as_str()
            }
            _ => "",
        };

        // Verifico si esoty actualizado
        let gov_id = approval_request.content.governance_id.clone();
        let governance = self.get_gov(ctx, gov_id).await?;

        match approval_request
            .content
            .gov_version
            .cmp(&governance.get_version())
        {
            std::cmp::Ordering::Equal => {
                // If it is the same it means that we have the latest version of governance, we are up to date.
            }
            std::cmp::Ordering::Greater => {
                // It is impossible to have a greater version of governance than the owner of the governance himself.
                // The only possibility is that it is an old approval request.
                // Hay que hacerlo TODO
            }
            std::cmp::Ordering::Less => {
                // Si es un sujeto de traabilidad hay que darle una vuelta.
                // Stop evaluation process, we need to update governance, we are out of date.
                // Hay que hacerlo TODO
            }
        }
        // identificador de la nueva request | forma de preguntar desde la api
        let id: DigestIdentifier =
            match DigestIdentifier::generate_with_blake3(&approval_request)
                .map_err(|_| Error::Approval("Error generating id".to_string()))
            {
                Ok(id) => id,
                Err(error) => return Err(error),
            };
        // actualizamos el estado del actor y ponemos en pendiente
        self.request = Some(ApprovalEntity {
            id: id.clone(),
            request: approval_request,
            state: ApprovalState::Pending,
        });
        Ok(())
    }
    // Nunca se va a poder emitir un voto para una request que no existe
    async fn generate_vote(
        &self,
        ctx: &mut ActorContext<Approver>,
        pending_request_id: &DigestIdentifier,
        acceptance: bool,
    ) -> Result<Signature, Error> {
        // Node path.
        let node_path = ActorPath::from("/user/node");
        // Node actor.
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        // We obtain the actor node
        let request_content = if let Some(request) = self.request.as_ref() {
            request.request.clone().content
        } else {
            return Err(Error::Actor("Request content is missing".into()));
        };

        let response = if let Some(node_actor) = node_actor {
            // We ask a node
            let response = node_actor
                .ask(NodeMessage::SignRequest(SignTypesNode::ApprovalReq(
                    request_content,
                )))
                .await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a node: {}",
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
        let response = match response {
            NodeResponse::SignRequest(sign) => Ok(sign),
            NodeResponse::Error(error) => Err(Error::Actor(format!(
                "The node encountered problems when signing the proof: {}",
                error
            ))),
            _ => Err(Error::Actor(format!(
                "An unexpected response has been received from node actor"
            ))),
        };
        response
    }
}

#[derive(Debug, Clone)]
pub enum ApproverCommand {
    LocalApprover {
        approval_req: Signed<ApprovalRequest>,
        our_key: KeyIdentifier,
    },
    NetworkApprover {
        request_id: String,
        approval_req: Signed<ApprovalRequest>,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    NetworkResponse {
        approval_res: Signed<ApprovalResponse>,
        request_id: String,
    },
    NetworkRequest {
        approval_req: Signed<ApprovalRequest>,
        info: ComunicateInfo,
    },
}

impl Message for ApproverCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApproverEvent {
    EmitVote { value: bool },
}

impl Event for ApproverEvent {}

#[derive(Debug, Clone)]
pub enum ApproverResponse {
    None,
}

impl Response for ApproverResponse {}

#[async_trait]
impl Actor for Approver {
    type Event = ApproverEvent;
    type Message = ApproverCommand;
    type Response = ApproverResponse;
}

#[async_trait]
impl Handler<Approver> for Approver {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: ApproverCommand,
        ctx: &mut ActorContext<Approver>,
    ) -> Result<ApproverResponse, ActorError> {
        match msg {
            ApproverCommand::LocalApprover {
                approval_req,
                our_key,
            } => {
                // generamos el estado
                if let Err(e) = self.approval(ctx, approval_req).await {
                    todo!()
                } else if self.pass_votation == VotationType::AlwaysAccept {
                    let request_id = match self.request.as_ref() {
                        Some(value) => value,
                        None => {
                            return Err(ActorError::Get(
                                "Request is missing".to_string(),
                            ));
                        }
                    };
                    // si se ha generado correctamente y aprobamos por defecto
                    let approval = match self
                        .generate_vote(
                            ctx,
                            &request_id.id,
                            true,
                        )
                        .await
                    {
                        Ok(approver) => ApprovalCommand::Response {
                            approval_res: ApprovalResponse::Signature(
                                approver, true,
                            ),
                            sender: our_key.clone(),
                        },
                        Err(e) => {
                            // Log con el error. TODO
                            ApprovalCommand::Response {
                                approval_res: ApprovalResponse::Error(
                                    ApprovalError {
                                        who: our_key.clone(),
                                        error: format!(
                                            "Error generating vote: {}",
                                            e
                                        ),
                                    },
                                ),
                                sender: our_key,
                            }
                        }
                    };
                    // Approval Path
                    let approval_path = ctx.path().parent();
                    // Approval actor.
                    let approval_actor: Option<ActorRef<Approval>> =
                        ctx.system().get_actor(&approval_path).await;
                    // Send response of validation to parent
                    if let Some(approval_actor) = approval_actor {
                        if let Err(e) = approval_actor.tell(approval).await {
                            return Err(e);
                        }
                    } else {
                        // Can not obtain parent actor
                        return Err(ActorError::Exists(approval_path));
                    }

                    ctx.stop().await;
                } else {
                    todo!()
                }
                todo!()
            }
            ApproverCommand::NetworkApprover {
                request_id,
                approval_req,
                node_key,
                our_key,
            } => {
                // Solo admitimos eventos FACT
                let subject_id = if let EventRequest::Fact(event) =
                    approval_req.content.event_request.content.clone()
                {
                    event.subject_id
                } else {
                    todo!()
                };
                let reciver_actor = 
                    format!(
                        "/user/node/{}/approver",
                        subject_id
                    );
                
                // Lanzar evento donde lanzar los retrys
                let message = NetworkMessage {
                    info: ComunicateInfo {
                        request_id,
                        sender: our_key,
                        reciver: node_key,
                        reciver_actor,
                        schema: "".to_string(),
                    },
                    message: ActorMessage::ApproverReq(approval_req),
                };
                let target = RetryNetwork::default();

                // Estrategia exponencial
                let strategy = Strategy::ExponentialBackoff(
                    ExponentialBackoffStrategy::new(
                        6,
                    )
                );

                let retry_actor = RetryActor::new(target, message, strategy);

                let retry = if let Ok(retry) = ctx
                    .create_child::<RetryActor<RetryNetwork>>(
                        "retry",
                        retry_actor,
                    )
                    .await
                {
                    retry
                } else {
                    todo!()
                };

                if let Err(e) = retry.tell(RetryMessage::Retry).await {
                    todo!()
                };

                todo!()
            }
            ApproverCommand::NetworkResponse {
                approval_res,
                request_id,
            } => !unimplemented!(),
            ApproverCommand::NetworkRequest { approval_req, info } => {
                !unimplemented!()
            }
        }
    }
}

#[async_trait]
impl PersistentActor for Approver {
    fn apply(&mut self, event: &ApproverEvent) {
        match event {
            ApproverEvent::EmitVote { value } => {
                
                
            }
        }
    }
}

impl Storable for Approver {}
