// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response, SystemEvent,
};

use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};

use crate::{intermediary::Intermediary, ActorMessage, NetworkMessage};

use super::authorizer::{self, Authorizer, AuthorizerMessage};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Authorization {
    subject_id: DigestIdentifier,
    our_key: KeyIdentifier,
    sn: u64,
    witnesses: HashSet<KeyIdentifier>,
    better: Option<KeyIdentifier>,
    schema_id: String,
    request: ActorMessage,
}

impl Authorization {
    pub fn new(
        subject_id: DigestIdentifier,
        our_key: KeyIdentifier,
        sn: u64,
        witnesses: HashSet<KeyIdentifier>,
        schema_id: String,
        request: ActorMessage,
    ) -> Self {
        Self {
            subject_id,
            our_key,
            sn,
            witnesses,
            better: None,
            schema_id,
            request,
        }
    }
    fn check_witness(&mut self, witness: KeyIdentifier) -> bool {
        self.witnesses.remove(&witness)
    }
}

#[derive(Debug, Clone)]
pub enum AuthorizationMessage {
    Create,
    Response { sender: KeyIdentifier, sn: u64 },
}

impl Message for AuthorizationMessage {}

#[derive(Debug, Clone)]
pub enum AuthorizationResponse {
    None,
}

impl Response for AuthorizationResponse {}

#[async_trait]
impl Actor for Authorization {
    type Event = ();
    type Message = AuthorizationMessage;
    type Response = AuthorizationResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        Ok(())
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        Ok(())
    }
}

#[async_trait]
impl Handler<Authorization> for Authorization {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: AuthorizationMessage,
        ctx: &mut ActorContext<Authorization>,
    ) -> Result<AuthorizationResponse, ActorError> {
        match msg {
            AuthorizationMessage::Create => {
                for witness in self.witnesses.clone() {
                    let authorizer = Authorizer::new(witness.clone());
                    let child = ctx
                        .create_child(&witness.to_string(), authorizer)
                        .await;
                    let Ok(child) = child else { todo!() };

                    if let Err(e) = child
                        .tell(AuthorizerMessage::NetworkLastSn {
                            subject_id: self.subject_id.clone(),
                            node_key: witness,
                            our_key: self.our_key.clone(),
                        })
                        .await
                    {
                        todo!()
                    }
                }
            }
            AuthorizationMessage::Response { sender, sn } => {
                if self.check_witness(sender.clone()) {
                    if sn > self.sn {
                        self.better = Some(sender);
                    }

                    if self.witnesses.is_empty() {
                        if let Some(node) = self.better.clone() {
                            let info = ComunicateInfo {
                                reciver: node,
                                sender: self.our_key.clone(),
                                request_id: String::default(),
                                reciver_actor: format!(
                                    "/user/node/{}/distributor",
                                    self.subject_id
                                ),
                                schema: self.schema_id.clone(),
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
                                            message: self.request.clone(),
                                        },
                                    },
                                )
                                .await
                            {
                                todo!()
                            };
                        }
                        ctx.stop().await;
                    }
                } else {
                    todo!()
                }
            }
        };

        Ok(AuthorizationResponse::None)
    }
}
