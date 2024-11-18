use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Handler,
    Message, Response,
};

use crate::Event as NetworkEvent;

use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Monitor;

impl Message for NetworkEvent {}

#[async_trait]
impl Actor for Monitor {
    type Message = NetworkEvent;
    type Event = ();
    type Response = ();

    async fn pre_start(
        &mut self,
        _ctx: &mut actor::ActorContext<Self>,
    ) -> Result<(), ActorError> {
        Ok(())
    }

    async fn pre_stop(
        &mut self,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        Ok(())
    }
}

#[async_trait]
impl Handler<Monitor> for Monitor {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: NetworkEvent,
        ctx: &mut actor::ActorContext<Monitor>,
    ) -> Result<(), ActorError> {
        Ok(())
    }
}
