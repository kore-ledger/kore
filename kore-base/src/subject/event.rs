// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    EventRequestType,
    model::common::{emit_fail, verify_protocols_state},
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{error, warn};

use crate::{
    Event as KoreEvent, EventRequest, Signed,
    approval::approver::{Approver, ApproverMessage},
    db::Storable,
};

const TARGET_EVENT: &str = "Kore-Subject-Event";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEvent {
    pub is_gov: bool,
    pub last_event: Option<Signed<KoreEvent>>,
}

impl LedgerEvent {
    pub fn new(is_gov: bool) -> Self {
        Self {
            is_gov,
            last_event: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LedgerEventMessage {
    UpdateLastEvent { event: Signed<KoreEvent> },
    GetLastEvent,
}

impl Message for LedgerEventMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEventEvent {
    pub event: Signed<KoreEvent>,
}

impl Event for LedgerEventEvent {}

#[derive(Debug, Clone)]
pub enum LedgerEventResponse {
    LastEvent(Signed<KoreEvent>),
    Ok,
}

impl Response for LedgerEventResponse {}

#[async_trait]
impl Actor for LedgerEvent {
    type Event = LedgerEventEvent;
    type Message = LedgerEventMessage;
    type Response = LedgerEventResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        let prefix = ctx.path().parent().key();
        self.init_store("event", Some(prefix), true, ctx).await
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<LedgerEvent> for LedgerEvent {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: LedgerEventMessage,
        ctx: &mut ActorContext<LedgerEvent>,
    ) -> Result<LedgerEventResponse, ActorError> {
        match msg {
            LedgerEventMessage::UpdateLastEvent { event } => {
                if let Some(last_event) = self.last_event.clone() {
                    if last_event.content.sn >= event.content.sn {
                        let e = "An attempt was made to update the event ledger with an event prior to the one already saved.";
                        warn!(TARGET_EVENT, "UpdateLastEvent, {}", e);
                        return Err(ActorError::Functional(e.to_owned()));
                    }
                };

                if let Err(e) = verify_protocols_state(
                    EventRequestType::from(
                        event.content.event_request.content.clone(),
                    ),
                    event.content.eval_success,
                    event.content.appr_success,
                    event.content.appr_required,
                    event.content.vali_success,
                    self.is_gov,
                ) {
                    warn!(TARGET_EVENT, "UpdateLastEvent, {}", e);
                    return Err(ActorError::Functional(e.to_string()));
                };

                self.on_event(
                    LedgerEventEvent {
                        event: event.clone(),
                    },
                    ctx,
                )
                .await;

                if self.is_gov {
                    if let EventRequest::EOL(_) =
                        event.content.event_request.content
                    {
                        return Ok(LedgerEventResponse::Ok);
                    } else {
                        let approver_path = ActorPath::from(format!(
                            "{}/approver",
                            ctx.path().parent()
                        ));
                        let approver_actor: Option<ActorRef<Approver>> =
                            ctx.system().get_actor(&approver_path).await;

                        if let Some(approver_actor) = approver_actor {
                            if let Err(e) = approver_actor
                                .tell(ApproverMessage::MakeObsolete)
                                .await
                            {
                                error!(
                                    TARGET_EVENT,
                                    "UpdateLastEvent, can not send message to Approver actor {}",
                                    e
                                );
                                return Err(emit_fail(ctx, e).await);
                            }
                        } else {
                            let e = ActorError::NotFound(approver_path);
                            warn!(
                                TARGET_EVENT,
                                "UpdateLastEvent, can not obtain Approver actor {}",
                                e
                            );
                            return Err(ActorError::Functional(e.to_string()));
                        }
                    };
                }

                Ok(LedgerEventResponse::Ok)
            }
            LedgerEventMessage::GetLastEvent => {
                let last_event = if let Some(last_event) =
                    self.last_event.clone()
                {
                    last_event
                } else {
                    warn!(TARGET_EVENT, "GetLastEvent, can not get last event");
                    return Err(ActorError::Functional(
                        "Can not get last event".to_owned(),
                    ));
                };

                Ok(LedgerEventResponse::LastEvent(last_event))
            }
        }
    }

    async fn on_event(
        &mut self,
        event: LedgerEventEvent,
        ctx: &mut ActorContext<LedgerEvent>,
    ) {
        if let Err(e) = self.persist_light(&event, ctx).await {
            error!(TARGET_EVENT, "OnEvent, can not persist information: {}", e);
            emit_fail(ctx, e).await;
        };

        if let Err(e) = ctx.publish_event(event).await {
            error!(TARGET_EVENT, "PublishEvent, can not publish event: {}", e);
            emit_fail(ctx, e).await;
        }
    }
}

#[async_trait]
impl PersistentActor for LedgerEvent {
    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        self.last_event = Some(event.event.clone());
        Ok(())
    }
}

impl Storable for LedgerEvent {}
