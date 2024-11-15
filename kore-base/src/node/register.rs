// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event,
    Handler, Message, Response,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use store::store::PersistentActor;

use crate::{db::Storable, request::state};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterData {
    pub subject_id: String,
    pub schema: String,
    pub active: bool
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Register {
    register_gov: HashMap<String, bool>,
    register_subj: HashMap<String, Vec<RegisterData>>
}

#[derive(Debug, Clone)]
pub enum RegisterMessage {
    RegisterGov {
        gov_id: String,
        active: bool
    },
    RegisterSubj {
        gov_id: String,
        data: RegisterData
    }
}

impl Message for RegisterMessage {}

#[derive(Debug, Clone)]
pub enum RegisterResponse {
    None,
}

impl Response for RegisterResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegisterEvent {
    RegisterGov {
        gov_id: String,
        active: bool
    },
    RegisterSubj {
        gov_id: String,
        data: RegisterData
    }
}

impl Event for RegisterEvent {}

#[async_trait]
impl Actor for Register {
    type Event = RegisterEvent;
    type Message = RegisterMessage;
    type Response = RegisterResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.init_store("register", None, false, ctx).await
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<Register> for Register {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: RegisterMessage,
        ctx: &mut actor::ActorContext<Register>,
    ) -> Result<RegisterResponse, ActorError> {
       match msg {
           RegisterMessage::RegisterGov { gov_id, active } => self.on_event(RegisterEvent::RegisterGov { gov_id, active }, ctx).await,
           RegisterMessage::RegisterSubj { gov_id, data } => self.on_event(RegisterEvent::RegisterSubj { gov_id, data }, ctx).await,
       }
        Ok(RegisterResponse::None)
    }

    async fn on_event(
        &mut self,
        event: RegisterEvent,
        ctx: &mut ActorContext<Register>,
    ) {
        if let Err(_e) = self.persist(&event, ctx).await {
            // TODO Propagar error.
        };
    }
}

#[async_trait]
impl PersistentActor for Register {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) {
        match event {
            RegisterEvent::RegisterGov { gov_id, active } => {
                self.register_gov.insert(gov_id.clone(), active.clone());
            },
            RegisterEvent::RegisterSubj { gov_id, data } => {
                self.register_subj
                    .entry(gov_id.clone())
                    .or_insert_with(Vec::new)
                    .push(data.clone());
            },
        }
    }
}

#[async_trait]
impl Storable for Register {}
