// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response, SystemEvent,
};
use async_trait::async_trait;
use authorization::{Authorization, AuthorizationMessage};
use identity::identifier::{
    derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
};
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    process::Child,
    str::FromStr,
};
use store::store::PersistentActor;

use crate::{
    db::Storable,
    error::Error,
    intermediary::Intermediary,
    model::common::{get_gov, get_metadata},
    subject::{self, Subject, SubjectMessage, SubjectResponse},
    ActorMessage, NetworkMessage,
};

pub mod authorization;
pub mod authorizer;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AuthWitness {
    One(KeyIdentifier),
    Many(Vec<KeyIdentifier>),
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Auth {
    our_node: KeyIdentifier,
    auth: HashMap<String, AuthWitness>,
}

impl Auth {
    pub fn new(key: KeyIdentifier) -> Self {
        Self {
            our_node: key,
            auth: HashMap::new(),
        }
    }

    async fn create_req_schema(
        ctx: &mut ActorContext<Auth>,
        subject_id: DigestIdentifier,
    ) -> Result<(u64, ActorMessage, String), Error> {
        'req: {
            let Ok(metadata) = get_metadata(ctx, &subject_id.to_string()).await
            else {
                break 'req;
            };
            let Ok(gov) = get_gov(ctx, &subject_id.to_string()).await else {
                // Debería tener la governanza, si tengo su metadata.
                todo!()
            };
            return Ok((
                metadata.sn,
                ActorMessage::DistributionLedgerReq {
                    gov_version: Some(gov.version),
                    actual_sn: Some(metadata.sn),
                    subject_id,
                },
                metadata.schema_id,
            ));
        }
        Ok((
            0,
            ActorMessage::DistributionLedgerReq {
                gov_version: None,
                actual_sn: None,
                subject_id,
            },
            String::default(),
        ))
    }
}

#[derive(Debug, Clone)]
pub enum AuthMessage {
    NewAuth {
        subject_id: DigestIdentifier,
        witness: AuthWitness,
    },
    GetAuths,
    GetAuth {
        subject_id: DigestIdentifier,
    },
    DeleteAuth {
        subject_id: DigestIdentifier,
    },
    Update {
        subject_id: DigestIdentifier,
    },
}

impl Message for AuthMessage {}

#[derive(Debug, Clone)]
pub enum AuthResponse {
    Auths { subjects: Vec<String> },
    Witnesses(AuthWitness),
    Error(Error),
    None,
}

impl Response for AuthResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthEvent {
    NewAuth {
        subject_id: String,
        witness: AuthWitness,
    },
    DeleteAuth {
        subject_id: String,
    },
}

impl Event for AuthEvent {}

#[async_trait]
impl Actor for Auth {
    type Event = AuthEvent;
    type Message = AuthMessage;
    type Response = AuthResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.init_store("auth", None, false, ctx).await
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<Auth> for Auth {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: AuthMessage,
        ctx: &mut actor::ActorContext<Auth>,
    ) -> Result<AuthResponse, ActorError> {
        match msg {
            AuthMessage::GetAuth { subject_id } => {
                if let Some(witnesses) = self.auth.get(&subject_id.to_string())
                {
                    return Ok(AuthResponse::Witnesses(witnesses.clone()));
                } else {
                    return Ok(AuthResponse::Error(Error::Auth(
                        "The subject has not been authorized".to_owned(),
                    )));
                }
            }
            AuthMessage::DeleteAuth { subject_id } => {
                self.on_event(
                    AuthEvent::DeleteAuth {
                        subject_id: subject_id.to_string(),
                    },
                    ctx,
                )
                .await;
            }
            AuthMessage::NewAuth {
                subject_id,
                witness,
            } => {
                self.on_event(
                    AuthEvent::NewAuth {
                        subject_id: subject_id.to_string(),
                        witness,
                    },
                    ctx,
                )
                .await;
            }
            AuthMessage::GetAuths => {
                return Ok(AuthResponse::Auths {
                    subjects: self.auth.keys().cloned().collect(),
                });
            }
            AuthMessage::Update { subject_id } => {
                let witness = self.auth.get(&subject_id.to_string());
                if let Some(witness) = witness {
                    let Ok((sn, request, schema_id)) =
                        Auth::create_req_schema(ctx, subject_id.clone()).await
                    else {
                        todo!()
                    };

                    match witness {
                        AuthWitness::One(key_identifier) => {
                            let info = ComunicateInfo {
                                reciver: key_identifier.clone(),
                                sender: self.our_node.clone(),
                                request_id: String::default(),
                                reciver_actor: format!(
                                    "/user/node/{}/distributor",
                                    subject_id
                                ),
                                schema: schema_id,
                            };

                            let helper: Option<Intermediary> =
                                ctx.system().get_helper("network").await;

                            let Some(mut helper) = helper else {
                                ctx.system()
                                    .send_event(SystemEvent::StopSystem)
                                    .await;
                                return Err(ActorError::NotHelper);
                            };

                            if let Err(_e) = helper
                                .send_command(
                                    network::CommandHelper::SendMessage {
                                        message: NetworkMessage {
                                            info,
                                            message: request,
                                        },
                                    },
                                )
                                .await
                            {
                                todo!()
                            };
                        }
                        AuthWitness::Many(vec) => {
                            let witnesses = vec.iter().cloned().collect();
                            let authorization = Authorization::new(
                                subject_id.clone(),
                                self.our_node.clone(),
                                sn,
                                witnesses,
                                schema_id,
                                request,
                            );
                            let child = ctx
                                .create_child(
                                    &subject_id.to_string(),
                                    authorization,
                                )
                                .await;
                            let child = match child {
                                Ok(child) => child,
                                Err(e) => todo!(),
                            };

                            if let Err(e) =
                                child.tell(AuthorizationMessage::Create).await
                            {
                                todo!()
                            }
                        }
                        AuthWitness::None => {
                            // Not Witness to update state of subject.
                            return Ok(AuthResponse::Error(Error::Auth("The subject has no witnesses to try to ask for an update.".to_owned())));
                        }
                    };
                } else {
                    return Ok(AuthResponse::Error(Error::Auth(
                        "The subject has not been authorized".to_owned(),
                    )));
                }
            }
        };

        Ok(AuthResponse::None)
    }

    async fn on_event(
        &mut self,
        event: AuthEvent,
        ctx: &mut ActorContext<Auth>,
    ) {
        if let Err(_e) = self.persist(&event, ctx).await {
            // TODO Propagar error.
        };
    }
}

#[async_trait]
impl PersistentActor for Auth {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) {
        match event {
            AuthEvent::NewAuth {
                subject_id,
                witness,
            } => {
                self.auth.insert(subject_id.clone(), witness.clone());
            }
            AuthEvent::DeleteAuth { subject_id } => {
                self.auth.remove(subject_id);
            }
        };
    }
}

#[async_trait]
impl Storable for Auth {}
