// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::Duration;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    FixedIntervalStrategy, Handler, Message, Response, RetryActor,
    RetryMessage, Strategy, SystemEvent,
};

use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};

use crate::{model::{common::emit_fail, network::RetryNetwork}, ActorMessage, NetworkMessage};

use super::authorization::{Authorization, AuthorizationMessage};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Authorizer {
    node: KeyIdentifier,
}

impl Authorizer {
    pub fn new(node: KeyIdentifier) -> Self {
        Self { node }
    }
}

#[derive(Debug, Clone)]
pub enum AuthorizerMessage {
    NetworkLastSn {
        subject_id: DigestIdentifier,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    NetworkResponse {
        sn: u64,
    },
}

impl Message for AuthorizerMessage {}

#[async_trait]
impl Actor for Authorizer {
    type Event = ();
    type Message = AuthorizerMessage;
    type Response = ();

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
impl Handler<Authorizer> for Authorizer {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: AuthorizerMessage,
        ctx: &mut ActorContext<Authorizer>,
    ) -> Result<(), ActorError> {
        match msg {
            AuthorizerMessage::NetworkLastSn {
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

                let retry = if let Ok(retry) = ctx
                    .create_child::<RetryActor<RetryNetwork>>(
                        "retry",
                        retry_actor,
                    )
                    .await
                {
                    retry
                } else {
                    let e = ActorError::Create(ctx.path().clone(), "retry".to_owned());
                    return Err(emit_fail(ctx, e).await);
                };

                if let Err(e) = retry.tell(RetryMessage::Retry).await {
                    return Err(emit_fail(ctx, e).await);
                };
            }
            AuthorizerMessage::NetworkResponse { sn } => {
                let authorization_path = ctx.path().parent();
                let authorization_actor: Option<ActorRef<Authorization>> =
                    ctx.system().get_actor(&authorization_path).await;

                
                if let Some(authorization_actor) = authorization_actor {
                    if let Err(e) = authorization_actor
                        .tell(AuthorizationMessage::Response {
                            sender: self.node.clone(),
                            sn,
                        })
                        .await
                    {
                        return Err(emit_fail(ctx, e).await);
                    }
                } else {
                    let e = ActorError::NotFound(authorization_path);
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

                    if let Err(_e) = retry.tell(RetryMessage::End).await {
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
        ctx: &mut ActorContext<Authorizer>,
    ) {
        match error {
            ActorError::ReTry => {
                let authorization_path = ctx.path().parent();

                // Evaluation actor.
                let authorization_actor: Option<ActorRef<Authorization>> =
                    ctx.system().get_actor(&authorization_path).await;

                if let Some(authorization_actor) = authorization_actor {
                    if let Err(e) = authorization_actor
                        .tell(AuthorizationMessage::Response {
                            sender: self.node.clone(),
                            sn: 0,
                        })
                        .await
                    {
                        emit_fail(ctx, e).await;
                    }
                } else {
                    let e = ActorError::NotFound(authorization_path);
                    emit_fail(ctx, e).await;
                }
                ctx.stop().await;
            },
            _ => {
                // TODO Error inesperado.
            }
        };
    }
}
