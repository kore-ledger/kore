use crate::{
    db::Storable,
    intermediary::Intermediary,
    model::{
        common::{emit_fail, get_gov, get_sign},
        network::{RetryNetwork, TimeOutResponse},
        SignTypesNode, TimeStamp,
    },
    ActorMessage, Error, EventRequest, NetworkMessage, Signed, Subject,
    SubjectMessage, SubjectResponse,
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    ExponentialBackoffStrategy, Handler, Message, Response, RetryActor,
    RetryMessage, Strategy, SystemEvent,
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

impl ApprovalStateRes {
    pub fn to_string(&self) -> String {
        match self {
            ApprovalStateRes::RespondedAccepted => {
                "RespondedAccepted".to_owned()
            }
            ApprovalStateRes::RespondedRejected => {
                "RespondedRejected".to_owned()
            }
            ApprovalStateRes::Obsolete => "Obsolete".to_owned(),
        }
    }
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

impl ApprovalState {
    pub fn to_string(&self) -> String {
        match self {
            ApprovalState::RespondedAccepted => "RespondedAccepted".to_owned(),
            ApprovalState::RespondedRejected => "RespondedRejected".to_owned(),
            ApprovalState::Obsolete => "Obsolete".to_owned(),
            ApprovalState::Pending => "Pending".to_owned(),
        }
    }
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Approver {
    node: KeyIdentifier,
    request_id: String,
    subject_id: String,
    pass_votation: VotationType,
    state: Option<ApprovalState>,
    request: Option<ApprovalReq>,
    info: Option<ComunicateInfo>,
}

impl Approver {
    pub fn new(
        request_id: String,
        node: KeyIdentifier,
        subject_id: String,
        pass_votation: VotationType,
    ) -> Self {
        Approver {
            node,
            request_id,
            subject_id,
            pass_votation,
            state: None,
            request: None,
            info: None,
        }
    }

    async fn check_governance(
        &self,
        ctx: &mut ActorContext<Approver>,
        subject_id: &str,
        gov_version: u64,
    ) -> Result<(), ActorError> {
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
    ) -> Result<(), ActorError> {
        let sign_type = SignTypesNode::ApprovalSignature(ApprovalSignature {
            request: request.clone(),
            response,
        });

        let signature = get_sign(ctx, sign_type).await?;

        if let Some(info) = self.info.clone() {
            let res = ApprovalRes::Response(signature, response);

            let signature = get_sign(
                ctx,
                SignTypesNode::ApprovalRes(Box::new(res.clone())),
            )
            .await?;

            let signed_response: Signed<ApprovalRes> = Signed {
                content: res,
                signature,
            };

            let helper: Option<Intermediary> =
                ctx.system().get_helper("network").await;
            let Some(mut helper) = helper else {
                return Err(ActorError::NotHelper("network".to_string()));
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

            if let Err(e) = helper
                .send_command(network::CommandHelper::SendMessage {
                    message: NetworkMessage {
                        info: new_info,
                        message: ActorMessage::ApprovalRes {
                            res: Box::new(signed_response),
                        },
                    },
                })
                .await {
                    return Err(emit_fail(ctx, e).await);
                };
        } else {
            // Approval Path
            let approval_path = ActorPath::from(format!(
                "/user/node/{}/approval",
                request.subject_id
            ));
            // Approval actor.
            let approval_actor: Option<ActorRef<Approval>> =
                ctx.system().get_actor(&approval_path).await;
            // Send response of validation to parent
            if let Some(approval_actor) = approval_actor {
                approval_actor
                    .tell(ApprovalMessage::Response {
                        approval_res: ApprovalRes::Response(
                            signature, response,
                        ),
                        sender: self.node.clone(),
                    })
                    .await?;
            } else {
                return Err(ActorError::NotFound(approval_path));
            };
        };

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ApproverMessage {
    MakeObsolete,
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
    ChangeState {
        subject_id: String,
        state: ApprovalState,
    },
    SafeState {
        request_id: String,
        subject_id: String,
        request: ApprovalReq,
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
    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        let prefix = ctx.path().parent().key();
        self.init_store("approver", Some(prefix), false, ctx).await
    }
    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }

    type Event = ApproverEvent;
    type Message = ApproverMessage;
    type Response = ApproverResponse;
}

#[async_trait]
impl Handler<Approver> for Approver {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: ApproverMessage,
        ctx: &mut ActorContext<Approver>,
    ) -> Result<ApproverResponse, ActorError> {
        match msg {
            ApproverMessage::MakeObsolete => {
                let state = if let Some(state) = self.state.clone() {
                    state
                } else {
                    return Ok(ApproverResponse::None);
                };

                if state == ApprovalState::Pending {
                    self.on_event(
                        ApproverEvent::ChangeState {
                            subject_id: self.subject_id.clone(),
                            state: ApprovalState::Obsolete,
                        },
                        ctx,
                    )
                    .await;
                }
            }
            ApproverMessage::ChangeResponse { response } => {
                let Some(state) = self.state.clone() else {
                    return Ok(ApproverResponse::None);
                };

                if state == ApprovalState::Pending {
                    if response == ApprovalStateRes::Obsolete {
                        self.on_event(
                            ApproverEvent::ChangeState {
                                subject_id: self.subject_id.clone(),
                                state: ApprovalState::Obsolete,
                            },
                            ctx,
                        )
                        .await;
                    } else {
                        let (response, state) =
                            if ApprovalStateRes::RespondedAccepted == response {
                                (true, ApprovalState::RespondedAccepted)
                            } else {
                                (false, ApprovalState::RespondedRejected)
                            };

                        let Some(approval_req) = self.request.clone() else {
                            return Ok(ApproverResponse::None);
                        };

                        if let Err(e) = self
                            .send_response(ctx, approval_req, response)
                            .await
                        {
                            return Err(emit_fail(ctx, e).await);
                        };

                        self.on_event(
                            ApproverEvent::ChangeState {
                                subject_id: self.subject_id.clone(),
                                state,
                            },
                            ctx,
                        )
                        .await;
                    }
                }
            }
            // aprobar si esta por defecto
            ApproverMessage::LocalApproval {
                request_id,
                approval_req,
                our_key,
            } => {
                if request_id != self.request_id {
                    if !approval_req.event_request.content.is_fact_event() {
                        let e = ActorError::FunctionalFail("An attempt is being made to approve an event that is not fact.".to_owned());
                        return Err(emit_fail(ctx, e).await);
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
                            Err(e) => {
                                return Err(emit_fail(ctx, e).await);
                            }
                        };

                        // Approval Path
                        let approval_path = ActorPath::from(format!(
                            "/user/node/{}/approval",
                            approval_req.subject_id
                        ));
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
                                return Err(emit_fail(ctx, e).await);
                            }
                        } else {
                            let e = ActorError::NotFound(approval_path);
                            return Err(emit_fail(ctx, e).await);
                        }

                        self.on_event(
                            ApproverEvent::SafeState {
                                subject_id: self.subject_id.clone(),
                                request_id,
                                request: approval_req,
                                state: ApprovalState::RespondedAccepted,
                                info: None,
                            },
                            ctx,
                        )
                        .await;
                    } else {
                        self.on_event(
                            ApproverEvent::SafeState {
                                subject_id: self.subject_id.clone(),
                                request_id,
                                request: approval_req,
                                state: ApprovalState::Pending,
                                info: None,
                            },
                            ctx,
                        )
                        .await;
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
                    let e = ActorError::FunctionalFail("An attempt is being made to approve an event that is not fact.".to_owned());
                    return Err(emit_fail(ctx, e).await);
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

                let Ok(retry) = ctx
                    .create_child::<RetryActor<RetryNetwork>>(
                        "retry",
                        retry_actor,
                    )
                    .await
                else {
                    let e = ActorError::Create(ctx.path().clone(), "retry".to_string());
                    return Err(emit_fail(ctx, e).await);
                };

                if let Err(e) = retry.tell(RetryMessage::Retry).await {
                    return Err(emit_fail(ctx, e).await);
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
                        return Ok(ApproverResponse::None);
                    }
                    if let Err(_e) = approval_res.verify() {
                        // Hay error criptográfico en la respuesta
                        return Ok(ApproverResponse::None);
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
                            return Err(emit_fail(ctx, e).await);
                        }
                    } else {
                        let e = ActorError::NotFound(approval_path);
                        return Err(emit_fail(ctx, e).await);
                    }

                    'retry: {
                        let Some(retry) = ctx
                            .get_child::<RetryActor<RetryNetwork>>("retry")
                            .await
                        else {
                            // Aquí me da igual, porque al parar este actor para el hijo
                            break 'retry;
                        };

                        if let Err(_e) = retry.tell(RetryMessage::End).await {
                            // Aquí me da igual, porque al parar este actor para el hijo
                            break 'retry;
                        };
                    }

                    ctx.stop().await;
                } else {
                    // TODO llegó una respuesta con una request_id que no es la que estamos esperando, no es válido.
                }
            }
            ApproverMessage::NetworkRequest { approval_req, info } => {
                let info_subject_path =
                    ActorPath::from(info.reciver_actor.clone()).parent().key();
                // Nos llegó una approvación donde en la request se indica un sujeto pero en el info otro
                // Posible ataque.
                if info_subject_path
                    != approval_req.content.subject_id.to_string()
                {
                    return Ok(ApproverResponse::None);
                }

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
                            Err(e) => {
                                return Err(emit_fail(ctx, e).await);
                            }
                        }
                    } else {
                        let e = ActorError::NotFound(subject_path);
                        return Err(emit_fail(ctx, e).await);
                    };

                    let subject_owner = match response {
                        SubjectResponse::Owner(owner) => owner,
                        _ => {
                            let e = ActorError::UnexpectedMessage(subject_path, "SubjectResponse::Owner".to_owned());
                            return Err(emit_fail(ctx, e).await);
                        }
                    };

                    if subject_owner != approval_req.signature.signer {
                        // Error nos llegó una evaluation req de un nodo el cual no es el dueño
                        return Ok(ApproverResponse::None);
                    }

                    if let Err(_e) = approval_req.verify() {
                        // Hay errores criptográficos
                        return Ok(ApproverResponse::None);
                    }

                    if !approval_req
                        .content
                        .event_request
                        .content
                        .is_fact_event()
                    {
                        return Ok(ApproverResponse::None);
                    }

                    if let Err(_e) = self
                        .check_governance(
                            ctx,
                            &approval_req.content.subject_id.to_string(),
                            approval_req.content.gov_version,
                        )
                        .await
                    {
                        let e =  ActorError::UnexpectedMessage(subject_path, "SubjectResponse::Owner".to_owned());
                        return Err(emit_fail(ctx, e).await);
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
                            return Err(emit_fail(ctx, e).await);
                        };
                        self.on_event(
                            ApproverEvent::SafeState {
                                subject_id: self.subject_id.clone(),
                                request_id: info.request_id,
                                request: approval_req.content,
                                state: ApprovalState::RespondedAccepted,
                                info: None,
                            },
                            ctx,
                        )
                        .await;
                    } else {
                        self.on_event(
                            ApproverEvent::SafeState {
                                subject_id: self.subject_id.clone(),
                                request_id: info.request_id.clone(),
                                request: approval_req.content,
                                state: ApprovalState::Pending,
                                info: Some(info),
                            },
                            ctx,
                        )
                        .await;
                    }
                } else {
                    let state = if let Some(state) = self.state.clone() {
                        state
                    } else {
                        // Si tiene un request debería tener un state
                        return Ok(ApproverResponse::None);
                    };
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
                            // Si tiene un request debería tener un state
                            return Ok(ApproverResponse::None);
                        };

                    if let Err(e) = self
                        .send_response(ctx, approval_req.clone(), response)
                        .await
                    {
                        return Err(emit_fail(ctx, e).await);
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
            //TODO
        };

        if let Err(e) = ctx.publish_event(event).await {
            // TODO
        };
    }

    async fn on_child_error(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Approver>,
    ) {
        match error {
            ActorError::ReTry => {
                let approval_path = ctx.path().parent();

                let approval_actor: Option<ActorRef<Approval>> =
                    ctx.system().get_actor(&approval_path).await;

                if let Some(approval_actor) = approval_actor {
                    if let Err(e) = approval_actor
                        .tell(ApprovalMessage::Response {
                            approval_res: ApprovalRes::TimeOut(
                                TimeOutResponse {
                                    re_trys: 3,
                                    timestamp: TimeStamp::now(),
                                    who: self.node.clone(),
                                },
                            ),
                            sender: self.node.clone(),
                        })
                        .await
                    {
                        emit_fail(ctx, e).await;
                    }
                } else {
                    let e = ActorError::NotFound(approval_path);
                    emit_fail(ctx, e).await;
                }
                ctx.stop().await;
            }
            _ => {
                // TODO Error inesperado o que no debería ocurrir.
            }
        };
    }
}

// Debemos persistir el estado de la petición hasta que se apruebe
#[async_trait]
impl PersistentActor for Approver {
    fn apply(&mut self, event: &ApproverEvent) {
        match event {
            ApproverEvent::ChangeState { state, .. } => {
                self.state = Some(state.clone());
            }
            ApproverEvent::SafeState {
                request,
                state,
                info,
                request_id,
                ..
            } => {
                self.request_id = request_id.clone();
                self.request = Some(request.clone());
                self.state = Some(state.clone());
                self.info.clone_from(info);
            }
        }
    }
}

impl Storable for Approver {}
