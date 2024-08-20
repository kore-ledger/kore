use actor::{Actor, ActorContext, Error as ActorError, Event, Handler, Message, Response};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::Error;

pub struct Compiler {}

impl Compiler {

}


#[derive(Debug, Clone)]
pub enum CompilerCommand {
    Run
}

impl Message for CompilerCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompilerEvent {}

impl Event for CompilerEvent {}

#[derive(Debug, Clone)]
pub enum CompilerResponse {
    Error(Error),
    None
}

impl Response for CompilerResponse {}

#[async_trait]
impl Actor for Compiler {
    type Event = CompilerEvent;
    type Message = CompilerCommand;
    type Response = CompilerResponse;
}

#[async_trait]
impl Handler<Compiler> for Compiler {
    async fn handle_message(
        &mut self,
        msg: CompilerCommand,
        ctx: &mut ActorContext<Compiler>,
    ) -> Result<CompilerResponse, ActorError> {
        Ok(CompilerResponse::None)
    }

    async fn on_event(
        &mut self,
        event: CompilerEvent,
        ctx: &mut ActorContext<Compiler>,
    ) {}
}