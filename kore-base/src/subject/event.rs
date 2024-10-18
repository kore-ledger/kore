use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;

use crate::{db::Storable, Error, Event as KoreEvent, Signed};

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
        sender: ActorPath,
        msg: LedgerEventMessage,
        ctx: &mut ActorContext<LedgerEvent>,
    ) -> Result<LedgerEventResponse, ActorError> {
        match msg {
            LedgerEventMessage::UpdateLastEvent { event } => {
                self.on_event(event, ctx).await;
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
