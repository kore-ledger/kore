use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::event;

use crate::{
    approval::approver::{Approver, ApproverMessage}, db::Storable, Error, Event as KoreEvent, EventRequest, Signed
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LedgerEvent {
    pub last_event: Option<Signed<KoreEvent>>,
}

#[derive(Debug, Clone)]
pub enum LedgerEventMessage {
    UpdateLastEvent { event: Signed<KoreEvent> },
    GetLastEvent,
}

impl Message for LedgerEventMessage {}

impl Event for Signed<KoreEvent> {}

#[derive(Debug, Clone)]
pub enum LedgerEventResponse {
    Error(Error),
    LastEvent(Signed<KoreEvent>),
    Ok,
}

impl Response for LedgerEventResponse {}

#[async_trait]
impl Actor for LedgerEvent {
    type Event = Signed<KoreEvent>;
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
                self.on_event(event.clone(), ctx).await;

                let approver_path = ActorPath::from(format!(
                    "{}/approver",
                    ctx.path().parent()
                ));
                let approver_actor: Option<ActorRef<Approver>> =
                    ctx.system().get_actor(&approver_path).await;

                if let EventRequest::EOL(_) = event.content.event_request.content {
                        // LLEga un evento de EOL, todo funciona bien, el sujeto va a bajar al approver,
                        // Por lo tanto este no se va a poder marcar como obsoleta el último mensaje.
                        // A no ser que como exección el approver lo baje este actor
                        // TODO.
                } else {
                    if let Some(approver_actor) = approver_actor {
                        if let Err(_e) =
                            approver_actor.tell(ApproverMessage::MakeObsolete).await
                        {
                            todo!()
                        }
                    } else {
                        // LLEga un evento de EOL, todo funciona bien, el sujeto va a bajar al approver,
                        // Por lo tanto este m
                        todo!()
                    }
                };

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
        event: Signed<KoreEvent>,
        ctx: &mut ActorContext<LedgerEvent>,
    ) {
        if let Err(err) = self.persist(&event, ctx).await {
            let _ = ctx.emit_error(err).await;
        };
    }
}

#[async_trait]
impl PersistentActor for LedgerEvent {
    fn apply(&mut self, event: &Signed<KoreEvent>) {
        self.last_event = Some(event.clone());
    }
}

impl Storable for LedgerEvent {}
