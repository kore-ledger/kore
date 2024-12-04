// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, ChildAction, Error as ActorError,
    Handler, Message,
};

use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};
use updater::{Updater, UpdaterMessage};

use crate::{
    intermediary::Intermediary,
    model::common::emit_fail,
    request::manager::{RequestManager, RequestManagerMessage},
    ActorMessage, NetworkMessage,
};

pub mod updater;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UpdateType {
    Auth,
    Request { id: String },
}

pub struct UpdateNew {
    pub subject_id: DigestIdentifier,
    pub our_key: KeyIdentifier,
    pub sn: u64,
    pub witnesses: HashSet<KeyIdentifier>,
    pub schema_id: String,
    pub request: ActorMessage,
    pub update_type: UpdateType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Update {
    subject_id: DigestIdentifier,
    our_key: KeyIdentifier,
    sn: u64,
    witnesses: HashSet<KeyIdentifier>,
    better: Option<KeyIdentifier>,
    schema_id: String,
    request: ActorMessage,
    update_type: UpdateType,
}

impl Update {
    pub fn new(data: UpdateNew) -> Self {
        Self {
            subject_id: data.subject_id,
            our_key: data.our_key,
            sn: data.sn,
            witnesses: data.witnesses,
            better: None,
            schema_id: data.schema_id,
            request: data.request,
            update_type: data.update_type,
        }
    }
    fn check_witness(&mut self, witness: KeyIdentifier) -> bool {
        self.witnesses.remove(&witness)
    }
}

#[derive(Debug, Clone)]
pub enum UpdateMessage {
    Create,
    Response { sender: KeyIdentifier, sn: u64 },
}

impl Message for UpdateMessage {}

#[async_trait]
impl Actor for Update {
    type Event = ();
    type Message = UpdateMessage;
    type Response = ();

    async fn pre_start(
        &mut self,
        _ctx: &mut ActorContext<Self>,
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
impl Handler<Update> for Update {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: UpdateMessage,
        ctx: &mut ActorContext<Update>,
    ) -> Result<(), ActorError> {
        match msg {
            UpdateMessage::Create => {
                for witness in self.witnesses.clone() {
                    let updater = Updater::new(witness.clone());
                    let child =
                        ctx.create_child(&witness.to_string(), updater).await;
                    let Ok(child) = child else {
                        let e = ActorError::Create(
                            ctx.path().clone(),
                            witness.to_string(),
                        );
                        return Err(emit_fail(ctx, e).await);
                    };

                    if let Err(e) = child
                        .tell(UpdaterMessage::NetworkLastSn {
                            subject_id: self.subject_id.clone(),
                            node_key: witness,
                            our_key: self.our_key.clone(),
                        })
                        .await
                    {
                        return Err(emit_fail(ctx, e).await);
                    }
                }
            }
            UpdateMessage::Response { sender, sn } => {
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
                                let e =
                                    ActorError::NotHelper("network".to_owned());
                                return Err(emit_fail(ctx, e).await);
                            };

                            if let Err(e) = helper
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
                                return Err(emit_fail(ctx, e).await);
                            };
                        }

                        if let UpdateType::Request { id } = &self.update_type {
                            let request_path = ActorPath::from(format!(
                                "/user/request/{}",
                                id
                            ));
                            let request_actor: Option<
                                ActorRef<RequestManager>,
                            > = ctx.system().get_actor(&request_path).await;

                            if let Some(request_actor) = request_actor {
                                let request = if self.better.is_some() {
                                    RequestManagerMessage::FinishReboot
                                } else {
                                    RequestManagerMessage::Reboot {
                                        governance_id: self.subject_id.clone(),
                                    }
                                };

                                if let Err(e) =
                                    request_actor.tell(request).await
                                {
                                    return Err(emit_fail(ctx, e).await);
                                }
                            } else {
                                let e = ActorError::NotFound(request_path);
                                return Err(emit_fail(ctx, e).await);
                            }
                        };

                        ctx.stop().await;
                    }
                }
            }
        };

        Ok(())
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Update>,
    ) -> ChildAction {
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}
