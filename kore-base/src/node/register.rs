// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use store::store::PersistentActor;
use tracing::{error, warn};

use crate::{db::Storable, model::common::emit_fail};

const TARGET_REGISTER: &str = "Kore-Node-Register";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterData {
    pub subject_id: String,
    pub schema: String,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GovsData {
    pub governance_id: String,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Register {
    register_gov: HashMap<String, bool>,
    register_subj: HashMap<String, Vec<RegisterData>>,
}

#[derive(Debug, Clone)]
pub enum RegisterMessage {
    GetGovs {
        active: Option<bool>,
    },
    GetSubj {
        gov_id: String,
        active: Option<bool>,
        schema: Option<String>,
    },
    RegisterGov {
        gov_id: String,
        active: bool,
    },
    RegisterSubj {
        gov_id: String,
        data: RegisterData,
    },
}

impl Message for RegisterMessage {}

#[derive(Debug, Clone)]
pub enum RegisterResponse {
    Govs { governances: Vec<GovsData> },
    Subjs { subjects: Vec<RegisterData> },
    None,
}

impl Response for RegisterResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegisterEvent {
    RegisterGov { gov_id: String, active: bool },
    RegisterSubj { gov_id: String, data: RegisterData },
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
            RegisterMessage::GetGovs { active } => {
                if let Some(active) = active {
                    return Ok(RegisterResponse::Govs {
                        governances: self
                            .register_gov
                            .iter()
                            .filter(|x| *x.1 == active)
                            .map(|x| GovsData {
                                active: *x.1,
                                governance_id: x.0.clone(),
                            })
                            .collect(),
                    });
                } else {
                    return Ok(RegisterResponse::Govs {
                        governances: self
                            .register_gov
                            .iter()
                            .map(|x| GovsData {
                                active: *x.1,
                                governance_id: x.0.clone(),
                            })
                            .collect(),
                    });
                }
            }
            RegisterMessage::GetSubj {
                gov_id,
                active,
                schema,
            } => {
                let subjects = self.register_subj.get(&gov_id.to_string());
                if let Some(subjects) = subjects {
                    let mut subj = vec![];
                    for subject in subjects {
                        if let Some(active) = active {
                            if subject.active != active {
                                continue;
                            };
                        };

                        if let Some(schema) = schema.clone() {
                            if subject.schema != schema {
                                continue;
                            }
                        }

                        subj.push(subject.clone());
                    }

                    return Ok(RegisterResponse::Subjs { subjects: subj });
                } else {
                    let e = "Governance id is not registered";
                    warn!(TARGET_REGISTER, "GetSubj, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                }
            }
            RegisterMessage::RegisterGov { gov_id, active } => {
                self.on_event(
                    RegisterEvent::RegisterGov { gov_id, active },
                    ctx,
                )
                .await
            }
            RegisterMessage::RegisterSubj { gov_id, data } => {
                self.on_event(RegisterEvent::RegisterSubj { gov_id, data }, ctx)
                    .await
            }
        }
        Ok(RegisterResponse::None)
    }

    async fn on_event(
        &mut self,
        event: RegisterEvent,
        ctx: &mut ActorContext<Register>,
    ) {
        if let Err(e) = self.persist_light(&event, ctx).await {
            error!(
                TARGET_REGISTER,
                "OnEvent, can not persist information: {}", e
            );
            emit_fail(ctx, e).await;
        };
    }
}

#[async_trait]
impl PersistentActor for Register {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        match event {
            RegisterEvent::RegisterGov { gov_id, active } => {
                self.register_gov.insert(gov_id.clone(), *active);
                self.register_subj.insert(gov_id.clone(), vec![]);
            }
            RegisterEvent::RegisterSubj { gov_id, data } => {
                self.register_subj
                    .entry(gov_id.clone())
                    .or_default()
                    .push(data.clone());
            }
        };

        Ok(())
    }
}

#[async_trait]
impl Storable for Register {}
