// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Handler, Message,
};

use crate::Event as NetworkEvent;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Actor in charge of monitoring the network, allows communication between the actor system and the network.
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
        _msg: NetworkEvent,
        _ctx: &mut actor::ActorContext<Monitor>,
    ) -> Result<(), ActorError> {
        Ok(())
    }
}
