// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use rush::{
    Actor, ActorContext, ActorError, ActorPath, Handler, Message, Response,
};

use crate::{Event as NetworkEvent, NetworkState};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Actor in charge of monitoring the network, allows communication between the actor system and the network.
pub struct Monitor {
    state: MonitorNetworkState,
}

impl Monitor {
    /// Monitor new
    pub fn new() -> Self {
        Self {
            state: MonitorNetworkState::default(),
        }
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Monitor network states
#[derive(Clone, Debug, Serialize, Deserialize, Default, Eq, PartialEq)]
pub enum MonitorNetworkState {
    /// Connecting to others network nodes
    #[default]
    Connecting,
    /// Connected to others netowrk nodes
    Running,
    /// Can not connect to others network nodes
    Down,
}

/// Monitor actor messages
#[derive(Debug, Clone)]
pub enum MonitorMessage {
    /// Network event
    Network(NetworkEvent),
    /// Network state
    State,
}

impl Message for MonitorMessage {}

/// Monitor actor responses
#[derive(Debug, Clone)]
pub enum MonitorResponse {
    /// Network state
    State(MonitorNetworkState),
    /// Defaulto message
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
        _ctx: &mut rush::ActorContext<Self>,
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
        _ctx: &mut rush::ActorContext<Monitor>,
    ) -> Result<MonitorResponse, ActorError> {
        match msg {
            MonitorMessage::Network(event) => {
                if let NetworkEvent::StateChanged(NetworkState::Running) = event
                {
                    self.state = MonitorNetworkState::Running
                }
                Ok(MonitorResponse::Ok)
            }
            MonitorMessage::State => {
                Ok(MonitorResponse::State(self.state.clone()))
            }
        }
    }
}
