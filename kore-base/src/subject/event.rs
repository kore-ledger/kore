use std::collections::HashSet;

use crate::{
    model::{common::verify_protocols_state, event::ProtocolsSignatures},
    EventRequestType,
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;

use crate::{
    approval::approver::{Approver, ApproverMessage},
    db::Storable,
    Error, Event as KoreEvent, EventRequest, Signed,
};

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
pub enum LedgerEventEvent {
    WithVal {
        event: Signed<KoreEvent>,
        validators: HashSet<KeyIdentifier>,
    },
    WithOutVal {
        event: Signed<KoreEvent>,
    },
}

impl Event for LedgerEventEvent {}

#[derive(Debug, Clone)]
pub enum LedgerEventResponse {
    Error(Error),
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
        self.init_store("event", Some(prefix), true, ctx).await?;

        Ok(())
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await.map_err(|_| ActorError::Stop)?;

        Ok(())
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
                        // NO Hay que actualizar
                        todo!()
                    }
                };

                let valid_event = match verify_protocols_state(
                    EventRequestType::from(
                        event.content.event_request.content.clone(),
                    ),
                    event.content.eval_success,
                    event.content.appr_success,
                    event.content.appr_required,
                    event.content.vali_success,
                ) {
                    Ok(is_ok) => is_ok,
                    Err(_e) => todo!(),
                };

                if valid_event {
                    let validators: HashSet<KeyIdentifier> =
                        if let Some(last_event) = self.last_event.clone() {
                            last_event
                                .content
                                .validators
                                .iter()
                                .map(|x| match x {
                                    ProtocolsSignatures::Signature(
                                        signature,
                                    ) => signature.signer.clone(),
                                    ProtocolsSignatures::TimeOut(
                                        time_out_response,
                                    ) => time_out_response.who.clone(),
                                })
                                .collect()
                        } else {
                            HashSet::new()
                        };

                    self.on_event(
                        LedgerEventEvent::WithVal {
                            event: event.clone(),
                            validators,
                        },
                        ctx,
                    )
                    .await;
                } else {
                    self.on_event(
                        LedgerEventEvent::WithOutVal {
                            event: event.clone(),
                        },
                        ctx,
                    )
                    .await;
                }

                if self.is_gov {
                    if let EventRequest::EOL(_) =
                        event.content.event_request.content
                    {
                        // LLEga un evento de EOL, todo funciona bien, el sujeto va a bajar al approver,
                        // Por lo tanto este no se va a poder marcar como obsoleta el último mensaje.
                        // A no ser que como exección el approver lo baje este actor
                        // TODO.
                    } else {
                        let approver_path = ActorPath::from(format!(
                            "{}/approver",
                            ctx.path().parent()
                        ));
                        let approver_actor: Option<ActorRef<Approver>> =
                            ctx.system().get_actor(&approver_path).await;

                        if let Some(approver_actor) = approver_actor {
                            if let Err(_e) = approver_actor
                                .tell(ApproverMessage::MakeObsolete)
                                .await
                            {
                                todo!()
                            }
                        } else {
                            todo!()
                        }
                    };
                }

                Ok(LedgerEventResponse::Ok)
            }
            LedgerEventMessage::GetLastEvent => {
                let last_event =
                    if let Some(last_event) = self.last_event.clone() {
                        last_event
                    } else {
                        todo!()
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
        if let Err(err) = self.persist(&event, ctx).await {
            let _ = ctx.emit_error(err).await;
        };

        if let Err(e) = ctx.publish_event(event).await {
            println!("{}", e);
            todo!()
        }
    }
}

#[async_trait]
impl PersistentActor for LedgerEvent {
    fn apply(&mut self, event: &LedgerEventEvent) {
        let event = match event {
            LedgerEventEvent::WithVal { event, .. } => event,
            LedgerEventEvent::WithOutVal { event } => event,
        };
        self.last_event = Some(event.clone());
    }
}

impl Storable for LedgerEvent {}
