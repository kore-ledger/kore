// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Handler, Message, Response,
};

use crate::Event as NetworkEvent;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Actor in charge of monitoring the network, allows communication between the actor system and the network.
pub struct Monitor {
    state: MonitorNetworkState
}

impl Monitor {
    /// Monitor new
    pub fn new() -> Self {
        Self { state: MonitorNetworkState::default() }
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum MonitorNetworkState {
    #[default]
    Connecting,
    Running,
    Down
}

#[derive(Debug, Clone)]
pub enum MonitorMessage {
    Network(NetworkEvent),
    State
}

impl Message for MonitorMessage {}



#[derive(Debug, Clone)]
pub enum MonitorResponse {
    State(MonitorNetworkState),
    Ok,
}

impl Response for MonitorResponse {}

#[async_trait]
impl Actor for Monitor {
    type Message = MonitorMessage;
    type Event = ();
    type Response = MonitorResponse;

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
        msg: MonitorMessage,
        _ctx: &mut actor::ActorContext<Monitor>,
    ) -> Result<MonitorResponse, ActorError> {
        match msg {            
            MonitorMessage::Network(event) => {
                if let NetworkEvent::Running = event {
                    self.state = MonitorNetworkState::Running
                }
                Ok(MonitorResponse::Ok)
            },
            MonitorMessage::State => Ok(MonitorResponse::State(self.state.clone())),
        }
    }
}
