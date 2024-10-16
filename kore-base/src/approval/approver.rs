use crate::{
    db::Storable,
    intermediary::Intermediary,
    model::{
        common::{get_gov, get_sign},
        network::RetryNetwork,
        SignTypesNode,
    },
    ActorMessage, Error, EventRequest, NetworkMessage, Signed, Subject,
    SubjectMessage, SubjectResponse,
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    ExponentialBackoffStrategy, Handler, Message, Response, RetryActor,
    RetryMessage, Strategy,
};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;

use super::{
    request::ApprovalReq,
    response::{ApprovalRes, ApprovalSignature},
    Approval, ApprovalMessage,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalStateRes {
    /// Request for approval which is in responded status and accepted
    RespondedAccepted,
    /// Request for approval which is in responded status and rejected
    RespondedRejected,
    /// The approval entity is obsolete.
    Obsolete,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalState {
    /// The approval entity is pending a response.
    #[default]
    Pending,
    /// Request for approval which is in responded status and accepted
    RespondedAccepted,
    /// Request for approval which is in responded status and rejected
    RespondedRejected,
    /// The approval entity is obsolete.
    Obsolete,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum VotationType {
    #[default]
    Manual,
    AlwaysAccept,
}

impl From<bool> for VotationType {
    fn from(passvotation: bool) -> Self {
        if passvotation {
            return Self::AlwaysAccept;
        }
        Self::Manual
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Approver {
    node: KeyIdentifier,
    request_id: String,
    pass_votation: VotationType,
    state: Option<ApprovalState>,
    request: Option<ApprovalReq>,
    info: Option<ComunicateInfo>,
}

impl Approver {
    pub fn new(request_id: String, node: KeyIdentifier) -> Self {
        Approver {
            node,
            request_id,
            ..Default::default()
        }
    }

    async fn check_governance(
        &self,
        ctx: &mut ActorContext<Approver>,
        subject_id: &str,
        gov_version: u64,
    ) -> Result<(), Error> {
        let governance = get_gov(ctx, subject_id).await?;

        match gov_version.cmp(&governance.version) {
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

        Ok(())
    }

    async fn send_response(
        &self,
        ctx: &mut ActorContext<Approver>,
        request: ApprovalReq,
        response: bool,
    ) -> Result<(), Error> {
        let sign_type = SignTypesNode::ApprovalSignature(ApprovalSignature {
            request: request.clone(),
            response,
        });

        let signature = match get_sign(ctx, sign_type).await {
            Ok(signature) => signature,
            Err(e) => todo!(),
        };

        let response = ApprovalRes::Response(signature, response);

        let helper: Option<Intermediary> =
            ctx.system().get_helper("NetworkIntermediary").await;
        let mut helper = if let Some(helper) = helper {
            helper
        } else {
            // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
            // return Err(ActorError::Get("Error".to_owned()))
            // return Err(ActorError::NotHelper);
            todo!()
        };

        let info = if let Some(info) = self.info.clone() {
            info
        } else {
            todo!()
        };

        let new_info = ComunicateInfo {
            reciver: info.sender,
            sender: info.reciver.clone(),
            request_id: info.request_id,
            reciver_actor: format!(
                "/user/node/{}/approval/{}",
                request.subject_id,
                info.reciver.clone()
            ),
            schema: info.schema.clone(),
        };

        let signature =
            match get_sign(ctx, SignTypesNode::ApprovalRes(response.clone()))
                .await
            {
                Ok(signature) => signature,
                Err(e) => todo!(),
            };

        let signed_response: Signed<ApprovalRes> = Signed {
            content: response,
            signature,
        };

        if let Err(e) = helper
            .send_command(network::CommandHelper::SendMessage {
                message: NetworkMessage {
                    info: new_info,
                    message: ActorMessage::ApprovalRes {
                        res: signed_response,
                    },
                },
            })
            .await
        {
            // error al enviar mensaje, propagar hacia arriba TODO
        };

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ApproverMessage {
    // Mensaje para aprobar localmente
    LocalApproval {
        request_id: String,
        approval_req: ApprovalReq,
        our_key: KeyIdentifier,
    },
    // Lanza los retries y envía la petición a la network(exponencial)
    NetworkApproval {
        request_id: String,
        approval_req: Signed<ApprovalReq>,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    // Finaliza los retries y recibe la respuesta de la network
    NetworkResponse {
        approval_res: Signed<ApprovalRes>,
        request_id: String,
    },
    // Mensaje para pedir aprobación desde el helper y devolver ahi
    NetworkRequest {
        approval_req: Signed<ApprovalReq>,
        info: ComunicateInfo,
    },
    ChangeResponse {
        response: ApprovalStateRes,
    }, // Necesito poder emitir un evento de aprobación, no solo el automático
}

impl Message for ApproverMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApproverEvent {
    SafeState {
        request: Option<ApprovalReq>,
        state: ApprovalState,
        info: Option<ComunicateInfo>,
    },
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
    type Message = ApproverMessage;
    type Response = ApproverResponse;
}

#[async_trait]
impl Handler<Approver> for Approver {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: ApproverMessage,
        ctx: &mut ActorContext<Approver>,
    ) -> Result<ApproverResponse, ActorError> {
        match msg {
            ApproverMessage::ChangeResponse { response } => {
                let state = if let Some(state) = self.state.clone() {
                    state
                } else {
                    todo!()
                };

                if state == ApprovalState::Pending {
                    if response == ApprovalStateRes::Obsolete {
                        self.on_event(ApproverEvent::SafeState {
                            request: self.request.clone(),
                            state: ApprovalState::Obsolete,
                            info: None,
                        }, ctx).await;
                    } else {
                        let (response, state) =
                            if ApprovalStateRes::RespondedAccepted == response {
                                (true, ApprovalState::RespondedAccepted)
                            } else {
                                (false, ApprovalState::RespondedRejected)
                            };

                        let approval_req =
                            if let Some(approval_req) = self.request.clone() {
                                approval_req
                            } else {
                                todo!()
                            };

                        if let Err(e) = self
                            .send_response(ctx, approval_req, response)
                            .await
                        {
                            todo!()
                        };

                        self.on_event(ApproverEvent::SafeState {
                            request: self.request.clone(),
                            state,
                            info: self.info.clone(),
                        }, ctx).await;
                    }
                } else {
                    todo!()
                }
            }
            // aprobar si esta por defecto
            ApproverMessage::LocalApproval {
                request_id,
                approval_req,
                our_key,
            } => {
                if request_id != self.request_id {
                    let EventRequest::Fact(state_data) =
                        &approval_req.event_request.content
                    else {
                        // Manejar, solo se evaluan los eventos de tipo fact TODO
                        todo!()
                    };

                    if let Err(e) = self
                        .check_governance(
                            ctx,
                            &approval_req.subject_id.to_string(),
                            approval_req.gov_version,
                        )
                        .await
                    {
                        todo!()
                    }

                    if self.pass_votation == VotationType::AlwaysAccept {
                        let sign_type = SignTypesNode::ApprovalSignature(
                            ApprovalSignature {
                                request: approval_req.clone(),
                                response: true,
                            },
                        );

                        let signature = match get_sign(ctx, sign_type).await {
                            Ok(signature) => signature,
                            Err(e) => todo!(),
                        };

                        // Approval Path
                        let approval_path = ctx.path().parent();
                        // Approval actor.
                        let approval_actor: Option<ActorRef<Approval>> =
                            ctx.system().get_actor(&approval_path).await;
                        // Send response of validation to parent
                        if let Some(approval_actor) = approval_actor {
                            if let Err(e) = approval_actor
                                .tell(ApprovalMessage::Response {
                                    approval_res: ApprovalRes::Response(
                                        signature, true,
                                    ),
                                    sender: our_key,
                                })
                                .await
                            {
                                return Err(e);
                            }
                        } else {
                            // Can not obtain parent actor
                            return Err(ActorError::Exists(approval_path));
                        }

                        self.on_event(ApproverEvent::SafeState {
                            request: Some(approval_req),
                            state: ApprovalState::RespondedAccepted,
                            info: None,
                        }, ctx).await;
                    } else {
                        self.on_event(ApproverEvent::SafeState {
                            request: Some(approval_req),
                            state: ApprovalState::Pending,
                            info: None,
                        }, ctx).await;
                    }
                }
            }
            ApproverMessage::NetworkApproval {
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
                    format!("/user/node/{}/approver", subject_id);

                // Lanzar evento donde lanzar los retrys
                let message = NetworkMessage {
                    info: ComunicateInfo {
                        request_id,
                        sender: our_key,
                        reciver: node_key,
                        reciver_actor,
                        schema: "".to_string(),
                    },
                    message: ActorMessage::ApprovalReq { req: approval_req },
                };

                let target = RetryNetwork::default();

                // Estrategia exponencial
                let strategy = Strategy::ExponentialBackoff(
                    ExponentialBackoffStrategy::new(6),
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
            }
            // Finaliza los retries
            ApproverMessage::NetworkResponse {
                approval_res,
                request_id,
            } => {
                if request_id == self.request_id {
                    if self.node != approval_res.signature.signer {
                        // Nos llegó a una aprobación de un nodo incorrecto!
                        todo!()
                    }
                    if let Err(e) = approval_res.verify() {
                        // Hay error criptográfico en la respuesta
                        todo!()
                    }

                    // Approval path.
                    let approval_path = ctx.path().parent();

                    // Approval actor.
                    let approval_actor: Option<ActorRef<Approval>> =
                        ctx.system().get_actor(&approval_path).await;

                    if let Some(approval_actor) = approval_actor {
                        if let Err(e) = approval_actor
                            .tell(ApprovalMessage::Response {
                                approval_res: approval_res.content,
                                sender: self.node.clone(),
                            })
                            .await
                        {
                            // TODO error, no se puede enviar la response. Parar
                        }
                    } else {
                        // TODO no se puede obtener aprobación! Parar.
                        // Can not obtain parent actor
                    }

                    let retry = if let Some(retry) =
                        ctx.get_child::<RetryActor<RetryNetwork>>("retry").await
                    {
                        retry
                    } else {
                        todo!()
                    };
                    if let Err(e) = retry.tell(RetryMessage::End).await {
                        todo!()
                    };
                    ctx.stop().await;
                } else {
                    // TODO llegó una respuesta con una request_id que no es la que estamos esperando, no es válido.
                }
            }
            ApproverMessage::NetworkRequest { approval_req, info } => {
                if info.request_id != self.request_id {
                    let subject_path = ActorPath::from(format!(
                        "/user/node/{}",
                        approval_req.content.subject_id
                    ));
                    let subject_actor: Option<ActorRef<Subject>> =
                        ctx.system().get_actor(&subject_path).await;

                    // We obtain the evaluator
                    let response = if let Some(subject_actor) = subject_actor {
                        match subject_actor.ask(SubjectMessage::GetOwner).await
                        {
                            Ok(response) => response,
                            Err(e) => todo!(),
                        }
                    } else {
                        todo!()
                    };

                    let subject_owner = match response {
                        SubjectResponse::Owner(owner) => owner,
                        _ => todo!(),
                    };

                    if subject_owner != approval_req.signature.signer {
                        // Error nos llegó una evaluation req de un nodo el cual no es el dueño
                        todo!()
                    }

                    if let Err(e) = approval_req.verify() {
                        // Hay errores criptográficos
                        todo!()
                    }

                    let helper: Option<Intermediary> =
                        ctx.system().get_helper("NetworkIntermediary").await;
                    let helper = if let Some(helper) = helper {
                        helper
                    } else {
                        // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
                        // return Err(ActorError::Get("Error".to_owned()))
                        return Err(ActorError::NotHelper);
                    };

                    let EventRequest::Fact(_state_data) =
                        &approval_req.content.event_request.content
                    else {
                        // Manejar, solo se aprueban los eventos de tipo fact TODO
                        todo!()
                    };

                    if let Err(e) = self
                        .check_governance(
                            ctx,
                            &approval_req.content.subject_id.to_string(),
                            approval_req.content.gov_version,
                        )
                        .await
                    {
                        todo!()
                    }

                    if self.pass_votation == VotationType::AlwaysAccept {
                        if let Err(e) = self
                            .send_response(
                                ctx,
                                approval_req.content.clone(),
                                true,
                            )
                            .await
                        {
                            todo!()
                        };
                        self.on_event(ApproverEvent::SafeState {
                            request: Some(approval_req.content),
                            state: ApprovalState::RespondedAccepted,
                            info: None,
                        }, ctx).await;
                    } else {
                        self.on_event(ApproverEvent::SafeState {
                            request: Some(approval_req.content),
                            state: ApprovalState::Pending,
                            info: Some(info),
                        }, ctx).await;
                    }
                } else {
                    let state = if let Some(state) = self.state.clone() {
                        state
                    } else {
                        todo!()
                    };
                    // TODO refactorizar, este código también se envía en network_req.
                    let response = if ApprovalState::RespondedAccepted == state
                    {
                        true
                    } else if ApprovalState::RespondedRejected == state {
                        false
                    } else {
                        return Ok(ApproverResponse::None);
                    };

                    let approval_req =
                        if let Some(approval_req) = self.request.clone() {
                            approval_req
                        } else {
                            todo!()
                        };

                    if let Err(e) = self
                        .send_response(ctx, approval_req.clone(), response)
                        .await
                    {
                        todo!()
                    };
                }
            }
        }
        Ok(ApproverResponse::None)
    }

    async fn on_event(
        &mut self,
        event: ApproverEvent,
        ctx: &mut ActorContext<Approver>,
    ) {
        if let Err(e) = self.persist(&event, ctx).await {
            // TODO error al persistir, propagar hacia arriba
        };
    }
}

// Debemos persistir el estado de la petición hasta que se apruebe
#[async_trait]
impl PersistentActor for Approver {
    fn apply(&mut self, event: &ApproverEvent) {
        match event {
            ApproverEvent::SafeState {
                request,
                state,
                info,
            } => {
                self.request = request.clone();
                self.state = Some(state.clone());
                self.info = info.clone();
            }
        }
    }
}

impl Storable for Approver {}
