// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, ChildAction, Error as ActorError, Event,
    Handler, Message, Response,
};
use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, vec};
use store::store::PersistentActor;
use tracing::{error, warn};

use crate::{
    db::Storable, governance::model::RoleTypes, intermediary::Intermediary, model::{common::{emit_fail, get_gov, get_metadata, subject_old}, Namespace}, update::{Update, UpdateMessage, UpdateNew, UpdateRes}, ActorMessage, NetworkMessage
};

const TARGET_AUTH: &str = "Kore-Auth";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AuthWitness {
    One(KeyIdentifier),
    Many(Vec<KeyIdentifier>),
    None,
}

impl AuthWitness {
    fn merge(self, other: AuthWitness) -> AuthWitness {
        match (self, other) {
            (AuthWitness::None, w) | (w, AuthWitness::None) => w,
            (AuthWitness::One(x), AuthWitness::One(y)) => AuthWitness::Many(vec![x, y]),
            (AuthWitness::One(x), AuthWitness::Many(mut y)) => {
                y.push(x);
                AuthWitness::Many(y)
            }
            (AuthWitness::Many(mut x), AuthWitness::One(y)) => {
                x.push(y);
                AuthWitness::Many(x)
            }
            (AuthWitness::Many(mut x), AuthWitness::Many(y)) => {
                x.extend(y);
                AuthWitness::Many(x)
            }
        }
    }
}

fn merge_options(opt1: Option<AuthWitness>, opt2: Option<AuthWitness>) -> Option<AuthWitness> {
    match (opt1, opt2) {
        (Some(w1), Some(w2)) => Some(w1.merge(w2)),
        (Some(w), None) | (None, Some(w)) => Some(w),
        (None, None) => None,
    }
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
    ) -> Result<(u64, ActorMessage), ActorError> {
        'req: {
            let Ok(metadata) = get_metadata(ctx, &subject_id.to_string()).await
            else {
                break 'req;
            };
            let gov = get_gov(ctx, &subject_id.to_string()).await?;

            return Ok((
                metadata.sn,
                ActorMessage::DistributionLedgerReq {
                    gov_version: Some(gov.version),
                    actual_sn: Some(metadata.sn),
                    subject_id,
                },
            ));
        }
        Ok((
            0,
            ActorMessage::DistributionLedgerReq {
                gov_version: None,
                actual_sn: None,
                subject_id,
            },
        ))
    }
}

#[derive(Debug, Clone)]
pub enum AuthMessage {
    CheckTransfer {
        subject_id: DigestIdentifier,
    },
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
        more_info: WitnessesAuth
    },
}

#[derive(Debug, Clone)]
pub enum WitnessesAuth {
    None,
    Owner(KeyIdentifier),
    Witnesses
}

impl Message for AuthMessage {}

#[derive(Debug, Clone)]
pub enum AuthResponse {
    Auths { subjects: Vec<String> },
    Witnesses(AuthWitness),
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
            AuthMessage::CheckTransfer { subject_id } => {
                let is_pending = subject_old(
                    ctx,
                    &subject_id.to_string(),
                )
                .await.map_err(|e| {
                    error!(TARGET_AUTH, "CheckTransfer, Could not determine if the node is the owner of the subject: {}", e);
                    ActorError::Functional(format!(
                        "An error has occurred: {}",
                        e
                    ))
                })?;

                if !is_pending {
                    let e = "A Check transfer is being sent for a subject that is not pending to confirm or reject event";
                    error!(TARGET_AUTH, "CheckTransfer, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                }

                let witness = self.auth.get(&subject_id.to_string());
                if let Some(witness) = witness {
                    let witnesses = match witness {
                        AuthWitness::One(key_identifier) => {
                            vec![key_identifier.clone()]
                        }
                        AuthWitness::Many(vec) => {
                            vec.clone()
                        }
                        AuthWitness::None => {
                            let e = "The subject has no witnesses to try to ask for an update.";
                            error!(TARGET_AUTH, "Update, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }
                    }.iter().cloned().collect();
                    let data = UpdateNew {
                        subject_id: subject_id.clone(),
                        our_key: self.our_node.clone(),
                        response: None,
                        witnesses,
                        request: None,
                        update_type: crate::update::UpdateType::Transfer,
                    };

                    let authorization = Update::new(data);
                    let child = ctx
                        .create_child(
                            &format!("transfer_{}", subject_id),
                            authorization,
                        )
                        .await;
                    let Ok(child) = child else {
                        let e = ActorError::Create(
                            ctx.path().clone(),
                            subject_id.to_string(),
                        );
                        return Err(e);
                    };

                    if let Err(e) = child.tell(UpdateMessage::Transfer).await {
                        return Err(emit_fail(ctx, e).await);
                    }
                } else {
                    let e = "The subject has not been authorized";
                    error!(TARGET_AUTH, "CheckTransfer, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                }
            }
            AuthMessage::GetAuth { subject_id } => {
                if let Some(witnesses) = self.auth.get(&subject_id.to_string())
                {
                    return Ok(AuthResponse::Witnesses(witnesses.clone()));
                } else {
                    let e = "The subject has not been authorized";
                    error!(TARGET_AUTH, "GetAuth, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
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
            AuthMessage::Update { subject_id, more_info } => {
                let more_witness = match more_info {
                    WitnessesAuth::None => None,
                    WitnessesAuth::Owner(key_identifier) => Some(AuthWitness::One(key_identifier)),
                    WitnessesAuth::Witnesses => {
                        match get_gov(ctx, &subject_id.to_string()).await {
                            Ok(gov) => {
                                let (witnesses, _) = gov.get_signers(RoleTypes::Witness, "governance", Namespace::new());
                                Some(AuthWitness::Many(Vec::from_iter(witnesses.iter().cloned())))
                            },
                            Err(e) => {
                                warn!(TARGET_AUTH, "Update, When attempting to update governance, the use of explicit witnesses within governance has been indicated, but no governance has been found: {}", e);
                                Some(AuthWitness::None)
                            },
                        }
                    },
                };

                let auth_witness = self.auth.get(&subject_id.to_string()).cloned();
                let witness = merge_options(more_witness, auth_witness);

                if let Some(witness) = witness {
                    let (sn, request) =
                        match Auth::create_req_schema(ctx, subject_id.clone())
                            .await
                        {
                            Ok(data) => data,
                            Err(e) => {
                                error!(
                                    TARGET_AUTH,
                                    "Update, can not obtain request, sn, schema_id: {}",
                                    e
                                );
                                return Err(emit_fail(ctx, e).await);
                            }
                        };

                    match witness {
                        AuthWitness::One(key_identifier) => {
                            let info = ComunicateInfo {
                                reciver: key_identifier.clone(),
                                sender: self.our_node.clone(),
                                request_id: String::default(),
                                version: 0,
                                reciver_actor: format!(
                                    "/user/node/{}/distributor",
                                    subject_id
                                )
                            };

                            let helper: Option<Intermediary> =
                                ctx.system().get_helper("network").await;

                            let Some(mut helper) = helper else {
                                error!(
                                    TARGET_AUTH,
                                    "Update, can not obtain network helper"
                                );
                                let e =
                                    ActorError::NotHelper("network".to_owned());
                                return Err(emit_fail(ctx, e).await);
                            };

                            if let Err(e) = helper
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
                                error!(
                                    TARGET_AUTH,
                                    "Update, can not send response to network: {}",
                                    e
                                );
                                return Err(emit_fail(ctx, e).await);
                            };
                        }
                        AuthWitness::Many(vec) => {
                            let witnesses = vec.iter().cloned().collect();
                            let data = UpdateNew {
                                subject_id: subject_id.clone(),
                                our_key: self.our_node.clone(),
                                response: Some(UpdateRes::Sn(sn)),
                                witnesses,
                                request: Some(request),
                                update_type: crate::update::UpdateType::Auth,
                            };

                            let updater = Update::new(data);
                            let child = ctx
                                .create_child(&subject_id.to_string(), updater)
                                .await;
                            let Ok(child) = child else {
                                let e = ActorError::Create(
                                    ctx.path().clone(),
                                    subject_id.to_string(),
                                );
                                return Err(e);
                            };

                            if let Err(e) =
                                child.tell(UpdateMessage::Create).await
                            {
                                return Err(emit_fail(ctx, e).await);
                            }
                        }
                        AuthWitness::None => {
                            let e = "The subject has no witnesses to try to ask for an update.";
                            error!(TARGET_AUTH, "Update, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }
                    };
                } else {
                    let e = "The subject has not been authorized";
                    error!(TARGET_AUTH, "Update, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
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
        if let Err(e) = self.persist_light(&event, ctx).await {
            error!(TARGET_AUTH, "OnEvent, can not persist information: {}", e);
            emit_fail(ctx, e).await;
        };
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Auth>,
    ) -> ChildAction {
        error!(TARGET_AUTH, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}

#[async_trait]
impl PersistentActor for Auth {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
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

        Ok(())
    }
}

#[async_trait]
impl Storable for Auth {}
