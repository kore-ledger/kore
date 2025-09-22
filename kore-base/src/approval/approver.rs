// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::VecDeque, fmt::Display, time::Duration};

use crate::{
    ActorMessage, EventRequest, NetworkMessage, Signed,
    db::Storable,
    governance::Governance,
    intermediary::Intermediary,
    model::{
        SignTypesNode, TimeStamp,
        common::{
            UpdateData, emit_fail, get_metadata, get_sign,
            update_ledger_network,
        },
        network::{RetryNetwork, TimeOutResponse},
    },
    subject::Subject,
};
use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;
use rush::{
    Actor, ActorContext, ActorError, ActorPath, ActorRef, ChildAction,
    CustomIntervalStrategy, Event, Handler, Message, RetryActor, RetryMessage,
    Strategy,
};
use rush::{LightPersistence, PersistentActor};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use super::{
    Approval, ApprovalMessage,
    request::ApprovalReq,
    response::{ApprovalRes, ApprovalSignature},
};

const TARGET_APPROVER: &str = "Kore-Approval-Approver";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalStateRes {
    /// Request for approval which is in responded status and accepted
    RespondedAccepted,
    /// Request for approval which is in responded status and rejected
    RespondedRejected,
    /// The approval entity is obsolete.
    Obsolete,
}

impl Display for ApprovalStateRes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            ApprovalStateRes::RespondedAccepted => {
                "RespondedAccepted".to_owned()
            }
            ApprovalStateRes::RespondedRejected => {
                "RespondedRejected".to_owned()
            }
            ApprovalStateRes::Obsolete => "Obsolete".to_owned(),
        };
        write!(f, "{}", string,)
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

impl Display for ApprovalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            ApprovalState::RespondedAccepted => "RespondedAccepted".to_owned(),
            ApprovalState::RespondedRejected => "RespondedRejected".to_owned(),
            ApprovalState::Obsolete => "Obsolete".to_owned(),
            ApprovalState::Pending => "Pending".to_owned(),
        };
        write!(f, "{}", string,)
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
    version: u64,
    subject_id: String,
    pass_votation: VotationType,
    state: Option<ApprovalState>,
    request: Option<ApprovalReq>,
    info: Option<ComunicateInfo>,
}

impl Approver {
    pub fn new(
        request_id: String,
        version: u64,
        node: KeyIdentifier,
        subject_id: String,
        pass_votation: VotationType,
    ) -> Self {
        Approver {
            node,
            request_id,
            version,
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
        governance_id: DigestIdentifier,
        gov_version: u64,
        our_node: KeyIdentifier,
    ) -> Result<(), ActorError> {
        let governance_string = governance_id.to_string();
        let metadata = get_metadata(ctx, &governance_string).await?;
        let governance = match Governance::try_from(metadata.properties.clone())
        {
            Ok(gov) => gov,
            Err(e) => {
                let e = format!(
                    "can not convert governance from properties: {}",
                    e
                );
                return Err(ActorError::FunctionalFail(e));
            }
        };

        match gov_version.cmp(&governance.version) {
            std::cmp::Ordering::Equal => {
                // If it is the same it means that we have the latest version of governance, we are up to date.
            }
            std::cmp::Ordering::Greater => {
                // Me llega una versión mayor a la mía.
                let data = UpdateData {
                    sn: metadata.sn,
                    gov_version: governance.version,
                    subject_id: governance_id,
                    our_node,
                    other_node: self.node.clone(),
                };
                update_ledger_network(ctx, data).await?;
            }
            std::cmp::Ordering::Less => {
                // TODO Por ahora no vamos hacer nada, pero esto quiere decir que el owner perdió el ledger
                // lo recuperó pero no recibió la última versión. Aquí se podría haber producido un fork.
                // Esto ocurre solo en la aprobación porque solo se realiza en las gobernanzas.
            }
        }

        Ok(())
    }

    async fn send_response(
        &self,
        ctx: &mut ActorContext<Approver>,
        request: ApprovalReq,
        response: bool,
        info: Option<ComunicateInfo>,
    ) -> Result<(), ActorError> {
        let sign_type = SignTypesNode::ApprovalSignature(ApprovalSignature {
            request: request.clone(),
            response,
        });

        let signature = get_sign(ctx, sign_type).await?;

        if let Some(info) = info {
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
                version: info.version,
                reciver_actor: format!(
                    "/user/node/{}/approval/{}",
                    request.subject_id,
                    info.reciver.clone()
                ),
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
                .await
            {
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
        version: u64,
        approval_req: ApprovalReq,
        our_key: KeyIdentifier,
    },
    // Lanza los retries y envía la petición a la network(exponencial)
    NetworkApproval {
        approval_req: Signed<ApprovalReq>,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    // Finaliza los retries y recibe la respuesta de la network
    NetworkResponse {
        approval_res: Signed<ApprovalRes>,
        request_id: String,
        version: u64,
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
        version: u64,
        subject_id: String,
        request: Box<ApprovalReq>,
        state: ApprovalState,
        info: Option<ComunicateInfo>,
    },
}

impl Event for ApproverEvent {}

#[async_trait]
impl Actor for Approver {
    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        if ctx.parent::<Subject>().await.is_some() {
            let prefix = ctx.path().parent().key();
            self.init_store("approver", Some(prefix), false, ctx).await
        } else {
            Ok(())
        }
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        if ctx.parent::<Subject>().await.is_some() {
            self.stop_store(ctx).await
        } else {
            Ok(())
        }
    }

    type Event = ApproverEvent;
    type Message = ApproverMessage;
    type Response = ();
}

#[async_trait]
impl Handler<Approver> for Approver {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: ApproverMessage,
        ctx: &mut ActorContext<Approver>,
    ) -> Result<(), ActorError> {
        match msg {
            ApproverMessage::MakeObsolete => {
                let state = if let Some(state) = self.state.clone() {
                    state
                } else {
                    return Ok(());
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
                    warn!(
                        TARGET_APPROVER,
                        "ChangeResponse, Can not obtain approval state"
                    );
                    return Err(ActorError::Functional(
                        "Can not get approval state".to_owned(),
                    ));
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
                            warn!(
                                TARGET_APPROVER,
                                "ChangeResponse, Can not obtain approval request"
                            );
                            return Err(ActorError::Functional(
                                "Can not get approval request".to_owned(),
                            ));
                        };

                        if let Err(e) = self
                            .send_response(
                                ctx,
                                approval_req,
                                response,
                                self.info.clone(),
                            )
                            .await
                        {
                            error!(
                                TARGET_APPROVER,
                                "ChangeResponse, Can not send approver response to approval actor"
                            );
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
                version,
                approval_req,
                our_key,
            } => {
                if request_id != self.request_id || version != self.version {
                    if !approval_req.event_request.content.is_fact_event() {
                        let e = "An attempt is being made to approve an event that is not fact.";
                        error!(TARGET_APPROVER, "LocalApproval, {}", e);
                        let e = ActorError::FunctionalFail(e.to_owned());
                        return Err(emit_fail(ctx, e).await);
                    }

                    let state = if self.pass_votation
                        == VotationType::AlwaysAccept
                    {
                        let sign_type = SignTypesNode::ApprovalSignature(
                            ApprovalSignature {
                                request: approval_req.clone(),
                                response: true,
                            },
                        );

                        let signature = match get_sign(ctx, sign_type).await {
                            Ok(signature) => signature,
                            Err(e) => {
                                error!(
                                    TARGET_APPROVER,
                                    "LocalApproval, can not sign approver response: {}",
                                    e
                                );
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
                                error!(
                                    TARGET_APPROVER,
                                    "LocalApproval, can not send approver response to approval actor: {}",
                                    e
                                );
                                return Err(emit_fail(ctx, e).await);
                            }
                        } else {
                            error!(
                                TARGET_APPROVER,
                                "LocalApproval, can not obtain approval actor"
                            );
                            let e = ActorError::NotFound(approval_path);
                            return Err(emit_fail(ctx, e).await);
                        }

                        ApprovalState::RespondedAccepted
                    } else {
                        ApprovalState::Pending
                    };

                    self.on_event(
                        ApproverEvent::SafeState {
                            subject_id: self.subject_id.clone(),
                            version,
                            request_id,
                            request: Box::new(approval_req),
                            state,
                            info: None,
                        },
                        ctx,
                    )
                    .await;
                }
            }
            ApproverMessage::NetworkApproval {
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
                    let e = "An attempt is being made to approve an event that is not fact.";
                    error!(
                        TARGET_APPROVER,
                        "NetworkApproval, can not send approver response to approval actor: {}",
                        e
                    );
                    let e = ActorError::FunctionalFail(e.to_owned());
                    return Err(emit_fail(ctx, e).await);
                };

                let reciver_actor =
                    format!("/user/node/{}/approver", subject_id);

                // Lanzar evento donde lanzar los retrys
                let message = NetworkMessage {
                    info: ComunicateInfo {
                        request_id: self.request_id.clone(),
                        version: self.version,
                        sender: our_key,
                        reciver: node_key,
                        reciver_actor,
                    },
                    message: ActorMessage::ApprovalReq { req: approval_req },
                };

                let target = RetryNetwork::default();

                // Estrategia exponencial
                let strategy = Strategy::CustomIntervalStrategy(
                    CustomIntervalStrategy::new(VecDeque::from([
                        Duration::from_secs(14400),
                        Duration::from_secs(28800),
                        Duration::from_secs(57600),
                    ])),
                );

                let retry_actor = RetryActor::new(target, message, strategy);

                let Ok(retry) = ctx
                    .create_child::<RetryActor<RetryNetwork>>(
                        "retry",
                        retry_actor,
                    )
                    .await
                else {
                    let e = ActorError::Create(
                        ctx.path().clone(),
                        "retry".to_string(),
                    );
                    error!(
                        TARGET_APPROVER,
                        "NetworkApproval, can not create retry actor: {}", e
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                if let Err(e) = retry.tell(RetryMessage::Retry).await {
                    error!(
                        TARGET_APPROVER,
                        "NetworkApproval, can not send retry message to retry actor: {}",
                        e
                    );
                    return Err(emit_fail(ctx, e).await);
                };
            }
            // Finaliza los retries
            ApproverMessage::NetworkResponse {
                approval_res,
                request_id,
                version,
            } => {
                if request_id == self.request_id && version == self.version {
                    if self.node != approval_res.signature.signer {
                        let e = "We received an approval from a node which we were not expecting to receive.";
                        error!(TARGET_APPROVER, "NetworkResponse, {}", e);
                        return Err(ActorError::Functional(e.to_owned()));
                    }
                    if let Err(e) = approval_res.verify() {
                        error!(
                            TARGET_APPROVER,
                            "NetworkResponse, Can not verify approval response signature: {}",
                            e
                        );
                        return Err(ActorError::Functional(format!(
                            "Can not verify approval response signature: {}",
                            e
                        )));
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
                            error!(
                                TARGET_APPROVER,
                                "NetworkResponse, can not send approver response to approval actor: {}",
                                e
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
                    } else {
                        error!(
                            TARGET_APPROVER,
                            "NetworkResponse, can not obtain approval actor"
                        );
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

                        if let Err(e) = retry.tell(RetryMessage::End).await {
                            error!(
                                TARGET_APPROVER,
                                "NetworkResponse, can not send end message to retry actor: {}",
                                e
                            );
                            // Aquí me da igual, porque al parar este actor para el hijo
                            break 'retry;
                        };
                    }

                    ctx.stop(None).await;
                } else {
                    warn!(
                        TARGET_APPROVER,
                        "NetworkResponse, A response has been received with a request id or a version different from the current one"
                    );
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
                    let e = "We received an approvation where the request indicates one subject but the info indicates another.";
                    error!(TARGET_APPROVER, "NetworkRequest, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                }

                if info.request_id != self.request_id
                    || info.version != self.version
                {
                    if let Err(e) = approval_req.verify() {
                        let e = format!(
                            "Can not verify signature of request: {}",
                            e
                        );
                        error!(TARGET_APPROVER, "NetworkRequest, {}", e);
                        return Err(ActorError::Functional(e.to_owned()));
                    }

                    if !approval_req
                        .content
                        .event_request
                        .content
                        .is_fact_event()
                    {
                        let e = "Only can approve fact requests";
                        error!(TARGET_APPROVER, "NetworkRequest, {}", e);
                        let e = ActorError::Functional(e.to_owned());
                        return Err(e);
                    }

                    if let Err(e) = self
                        .check_governance(
                            ctx,
                            approval_req.content.subject_id.clone(),
                            approval_req.content.gov_version,
                            info.reciver.clone(),
                        )
                        .await
                    {
                        error!(
                            TARGET_APPROVER,
                            "NetworkRequest, can not obtain governance: {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }

                    let state = if self.pass_votation
                        == VotationType::AlwaysAccept
                    {
                        if let Err(e) = self
                            .send_response(
                                ctx,
                                approval_req.content.clone(),
                                true,
                                Some(info.clone()),
                            )
                            .await
                        {
                            error!(
                                TARGET_APPROVER,
                                "NetworkRequest, can not send approver response: {}",
                                e
                            );
                            return Err(emit_fail(ctx, e).await);
                        };

                        ApprovalState::RespondedAccepted
                    } else {
                        ApprovalState::Pending
                    };

                    self.on_event(
                        ApproverEvent::SafeState {
                            subject_id: self.subject_id.clone(),
                            request_id: info.request_id.clone(),
                            version: info.version,
                            request: Box::new(approval_req.content),
                            state,
                            info: Some(info),
                        },
                        ctx,
                    )
                    .await;
                } else if !self.request_id.is_empty() {
                    let state = if let Some(state) = self.state.clone() {
                        state
                    } else {
                        error!(
                            TARGET_APPROVER,
                            "NetworkRequest, can not obtain approval state"
                        );
                        let e = ActorError::FunctionalFail(
                            "Can not get state".to_owned(),
                        );
                        return Err(emit_fail(ctx, e).await);
                    };
                    let response = if ApprovalState::RespondedAccepted == state
                    {
                        true
                    } else if ApprovalState::RespondedRejected == state {
                        false
                    } else {
                        return Ok(());
                    };

                    let approval_req = if let Some(approval_req) =
                        self.request.clone()
                    {
                        approval_req
                    } else {
                        error!(
                            TARGET_APPROVER,
                            "NetworkRequest, can not obtain approval request"
                        );
                        let e = ActorError::FunctionalFail(
                            "Can not get approve request".to_owned(),
                        );
                        return Err(emit_fail(ctx, e).await);
                    };

                    if let Err(e) = self
                        .send_response(
                            ctx,
                            approval_req.clone(),
                            response,
                            Some(info),
                        )
                        .await
                    {
                        error!(
                            TARGET_APPROVER,
                            "NetworkRequest, can not send approval response"
                        );
                        return Err(emit_fail(ctx, e).await);
                    };
                }
            }
        }
        Ok(())
    }

    async fn on_event(
        &mut self,
        event: ApproverEvent,
        ctx: &mut ActorContext<Approver>,
    ) {
        if let Err(e) = self.persist(&event, ctx).await {
            error!(
                TARGET_APPROVER,
                "OnEvent, can not persist information: {}", e
            );
            emit_fail(ctx, e).await;
        };

        if let Err(e) = ctx.publish_event(event).await {
            error!(
                TARGET_APPROVER,
                "PublishEvent, can not publish event: {}", e
            );
            emit_fail(ctx, e).await;
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
                        error!(
                            TARGET_APPROVER,
                            "OnChildError, can not send response to Approval actor: {}",
                            e
                        );
                        emit_fail(ctx, e).await;
                    }
                } else {
                    let e = ActorError::NotFound(approval_path);
                    error!(
                        TARGET_APPROVER,
                        "OnChildError, can not obtain Approval actor: {}", e
                    );
                    emit_fail(ctx, e).await;
                }
                ctx.stop(None).await;
            }
            _ => {
                error!(TARGET_APPROVER, "OnChildError, unexpected error");
            }
        };
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Approver>,
    ) -> ChildAction {
        error!(TARGET_APPROVER, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}

// Debemos persistir el estado de la petición hasta que se apruebe
#[async_trait]
impl PersistentActor for Approver {
    type Persistence = LightPersistence;

    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        match event {
            ApproverEvent::ChangeState { state, .. } => {
                self.state = Some(state.clone());
            }
            ApproverEvent::SafeState {
                request,
                state,
                info,
                request_id,
                version,
                ..
            } => {
                self.version = *version;
                self.request_id.clone_from(request_id);
                self.request = Some(*request.clone());
                self.state = Some(state.clone());
                self.info.clone_from(info);
            }
        };

        Ok(())
    }
}

impl Storable for Approver {}
