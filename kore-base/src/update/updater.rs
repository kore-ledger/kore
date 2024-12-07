// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::Duration;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, ChildAction, Error as ActorError,
    FixedIntervalStrategy, Handler, Message, RetryActor, RetryMessage,
    Strategy,
};

use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::{
    model::{common::emit_fail, network::RetryNetwork},
    ActorMessage, NetworkMessage,
};

use super::{Update, UpdateMessage};

const TARGET_UPDATER: &str = "Kore-Update-Updater";

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Updater {
    node: KeyIdentifier,
}

impl Updater {
    pub fn new(node: KeyIdentifier) -> Self {
        Self { node }
    }
}

#[derive(Debug, Clone)]
pub enum UpdaterMessage {
    NetworkLastSn {
        subject_id: DigestIdentifier,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    NetworkResponse {
        sn: u64,
    },
}

impl Message for UpdaterMessage {}

#[async_trait]
impl Actor for Updater {
    type Event = ();
    type Message = UpdaterMessage;
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
impl Handler<Updater> for Updater {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: UpdaterMessage,
        ctx: &mut ActorContext<Updater>,
    ) -> Result<(), ActorError> {
        match msg {
            UpdaterMessage::NetworkLastSn {
                subject_id,
                node_key,
                our_key,
            } => {
                let message = NetworkMessage {
                    info: ComunicateInfo {
                        request_id: String::default(),
                        sender: our_key,
                        reciver: node_key,
                        reciver_actor: format!(
                            "/user/node/{}/distributor",
                            subject_id
                        ),
                        schema: String::default(),
                    },
                    message: ActorMessage::DistributionGetLastSn { subject_id },
                };

                let target = RetryNetwork::default();

                let strategy = Strategy::FixedInterval(
                    FixedIntervalStrategy::new(1, Duration::from_secs(3)),
                );

                let retry_actor = RetryActor::new(target, message, strategy);

                let retry = match ctx
                    .create_child::<RetryActor<RetryNetwork>>(
                        "retry",
                        retry_actor,
                    )
                    .await
                {
                    Ok(retry) => retry,
                    Err(e) => {
                        error!(TARGET_UPDATER, "NetworkLastSn, can not create Retry actor: {}", e);
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                if let Err(e) = retry.tell(RetryMessage::Retry).await {
                    error!(TARGET_UPDATER, "NetworkLastSn, can not send retry to Retry actor: {}", e);
                    return Err(emit_fail(ctx, e).await);
                };
            }
            UpdaterMessage::NetworkResponse { sn } => {
                let update_path = ctx.path().parent();
                let update_actor: Option<ActorRef<Update>> =
                    ctx.system().get_actor(&update_path).await;

                if let Some(update_actor) = update_actor {
                    if let Err(e) = update_actor
                        .tell(UpdateMessage::Response {
                            sender: self.node.clone(),
                            sn,
                        })
                        .await
                    {
                        error!(TARGET_UPDATER, "NetworkResponse, can not send response to Update actor: {}", e);
                        return Err(emit_fail(ctx, e).await);
                    }
                } else {
                    let e = ActorError::NotFound(update_path);
                    error!(TARGET_UPDATER, "NetworkResponse, can not obtain Update actor: {}", e);
                    return Err(emit_fail(ctx, e).await);
                }

                'retry: {
                    let Some(retry) = ctx
                        .get_child::<RetryActor<RetryNetwork>>("retry")
                        .await
                    else {
                        // Aquí me da igual, porque al parar este actor para el hijo
                        break 'retry;
                    };

                    if let Err(e) = retry.tell(RetryMessage::End).await {
                        warn!(TARGET_UPDATER, "NetworkResponse, can not end Retry actor: {}", e);
                        // Aquí me da igual, porque al parar este actor para el hijo
                        break 'retry;
                    };
                }

                ctx.stop().await;
            }
        };

        Ok(())
    }

    async fn on_child_error(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Updater>,
    ) {
        match error {
            ActorError::ReTry => {
                let update_path = ctx.path().parent();

                // Evaluation actor.
                let update_actor: Option<ActorRef<Update>> =
                    ctx.system().get_actor(&update_path).await;

                if let Some(update_actor) = update_actor {
                    if let Err(e) = update_actor
                        .tell(UpdateMessage::Response {
                            sender: self.node.clone(),
                            sn: 0,
                        })
                        .await
                    {
                        error!(TARGET_UPDATER, "OnChildError, can not send response to Update actor: {}", e);
                        emit_fail(ctx, e).await;
                    }
                } else {
                    let e = ActorError::NotFound(update_path);
                    error!(TARGET_UPDATER, "OnChildError, can not obtain Update actor: {}", e);
                    emit_fail(ctx, e).await;
                }
                ctx.stop().await;
            }
            _ => {
                error!(TARGET_UPDATER, "OnChildError, unexpected error");
            }
        };
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Updater>,
    ) -> ChildAction {
        error!(TARGET_UPDATER, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}
