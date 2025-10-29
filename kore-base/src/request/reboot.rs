

use std::time::Duration;

use async_trait::async_trait;
use identity::identifier::DigestIdentifier;
use rush::{Actor, ActorContext, ActorError, ActorPath, Handler, Message};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::model::common::{emit_fail, get_last_event, get_metadata};

use super::manager::{RequestManager, RequestManagerMessage};

const TARGET_REBOOT: &str = "Kore-Request-Reboot";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reboot {
    governance_id: DigestIdentifier,
    sn_ledger: u64,
    sn_event: u64,
}

impl Reboot {
    pub fn new(governance_id: DigestIdentifier) -> Self {
        Self {
            governance_id,
            sn_ledger: 0,
            sn_event: 0,
        }
    }

    async fn update_event_sn(
        &mut self,
        ctx: &mut rush::ActorContext<Reboot>,
    ) -> Result<(), ActorError> {
        self.sn_event =
            match get_last_event(ctx, &self.governance_id.to_string()).await {
                Ok(last_event) => last_event.content.sn,
                Err(e) => {
                    if let ActorError::Functional(_) = e {
                        0
                    } else {
                        return Err(e);
                    }
                }
            };
        Ok(())
    }

    async fn update_ledger_sn(
        &mut self,
        ctx: &mut rush::ActorContext<Reboot>,
    ) -> Result<(), ActorError> {
        let metadata =
            get_metadata(ctx, &self.governance_id.to_string()).await?;
        self.sn_ledger = metadata.sn;

        Ok(())
    }

    async fn sleep(
        &self,
        ctx: &mut rush::ActorContext<Reboot>,
    ) -> Result<(), ActorError> {
        let actor = ctx.reference().await;
        if let Some(actor) = actor {
            let request = RebootMessage::Update {
                last_sn_event: self.sn_event,
                last_sn_ledger: self.sn_ledger,
            };
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if let Err(e) = actor.tell(request).await {
                    error!(
                        TARGET_REBOOT,
                        "Sleep, can not send Update message to Reboot actor: {}",
                        e
                    );
                }
            });
        } else {
            let path = ctx.path().clone();
            return Err(ActorError::NotFound(path));
        }
        Ok(())
    }

    async fn finish(
        ctx: &mut rush::ActorContext<Reboot>,
    ) -> Result<(), ActorError> {
        let request_actor: Option<rush::ActorRef<RequestManager>> =
            ctx.parent().await;

        if let Some(request_actor) = request_actor {
            request_actor
                .tell(RequestManagerMessage::FinishReboot)
                .await?
        } else {
            let path = ctx.path().parent();
            return Err(ActorError::NotFound(path));
        }

        ctx.stop(None).await;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RebootMessage {
    Init,
    Update {
        last_sn_event: u64,
        last_sn_ledger: u64,
    },
}

impl Message for RebootMessage {}

#[async_trait]
impl Actor for Reboot {
    type Message = RebootMessage;
    type Event = ();
    type Response = ();

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
impl Handler<Reboot> for Reboot {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: RebootMessage,
        ctx: &mut rush::ActorContext<Reboot>,
    ) -> Result<(), ActorError> {
        match msg {
            RebootMessage::Init => {
                if let Err(e) = self.update_event_sn(ctx).await {
                    error!(
                        TARGET_REBOOT,
                        "Init, can not uptade event sn: {}", e
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                if let Err(e) = self.update_ledger_sn(ctx).await {
                    error!(
                        TARGET_REBOOT,
                        "Init, can not uptade ledger sn: {}", e
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                if let Err(e) = self.sleep(ctx).await {
                    error!(TARGET_REBOOT, "Init, can not sleep: {}", e);
                    return Err(emit_fail(ctx, e).await);
                };
            }
            RebootMessage::Update {
                last_sn_event,
                last_sn_ledger,
            } => {
                if let Err(e) = self.update_event_sn(ctx).await {
                    error!(
                        TARGET_REBOOT,
                        "Update, can not uptade event sn: {}", e
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                if self.sn_event > last_sn_event {
                    if let Err(e) = Self::finish(ctx).await {
                        error!(
                            TARGET_REBOOT,
                            "Update, can not finish reboot: {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                    return Ok(());
                }

                if let Err(e) = self.update_ledger_sn(ctx).await {
                    error!(
                        TARGET_REBOOT,
                        "Update, can not uptade ledger sn: {}", e
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                if self.sn_ledger > last_sn_ledger
                    && let Err(e) = self.sleep(ctx).await
                {
                    error!(TARGET_REBOOT, "Update, can not sleep: {}", e);
                    return Err(emit_fail(ctx, e).await);
                }

                if let Err(e) = Self::finish(ctx).await {
                    error!(
                        TARGET_REBOOT,
                        "Update, can not finish reboot: {}", e
                    );
                    return Err(emit_fail(ctx, e).await);
                }
            }
        };

        Ok(())
    }
}
