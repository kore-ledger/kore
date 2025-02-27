use std::collections::HashSet;

use crate::model::common::emit_fail;
use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response,
};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::error;

use crate::db::Storable;

const TARGET_VALIDATA: &str = "Kore-Subject-TransferRegister";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransferRegister {
    old_owners: HashSet<KeyIdentifier>,
}

#[derive(Debug, Clone)]
pub enum TransferRegisterMessage {
    RegisterNewOldOwner {
        old: KeyIdentifier,
        new: KeyIdentifier,
    },
    IsOldOwner(KeyIdentifier),
}

impl Message for TransferRegisterMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferRegisterEvent {
    RegisterNewOldOwner {
        old: KeyIdentifier,
        new: KeyIdentifier,
    },
}

impl Event for TransferRegisterEvent {}

pub enum TransferRegisterResponse {
    IsOwner(bool),
    Ok,
}

impl Response for TransferRegisterResponse {}

#[async_trait]
impl Actor for TransferRegister {
    type Event = TransferRegisterEvent;
    type Message = TransferRegisterMessage;
    type Response = TransferRegisterResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        let prefix = ctx.path().parent().key();
        self.init_store("transfer_register", Some(prefix), true, ctx)
            .await
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<TransferRegister> for TransferRegister {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: TransferRegisterMessage,
        ctx: &mut ActorContext<TransferRegister>,
    ) -> Result<TransferRegisterResponse, ActorError> {
        match msg {
            TransferRegisterMessage::RegisterNewOldOwner { old, new } => {
                self.on_event(
                    TransferRegisterEvent::RegisterNewOldOwner { old, new },
                    ctx,
                )
                .await
            }
            TransferRegisterMessage::IsOldOwner(old) => {
                return Ok(TransferRegisterResponse::IsOwner(
                    self.old_owners.contains(&old),
                ));
            }
        };

        Ok(TransferRegisterResponse::Ok)
    }

    async fn on_event(
        &mut self,
        event: TransferRegisterEvent,
        ctx: &mut ActorContext<TransferRegister>,
    ) {
        if let Err(e) = self.persist_light(&event, ctx).await {
            error!(
                TARGET_VALIDATA,
                "OnEvent, can not persist information: {}", e
            );
            emit_fail(ctx, e).await;
        };
    }
}

#[async_trait]
impl PersistentActor for TransferRegister {
    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        match event {
            TransferRegisterEvent::RegisterNewOldOwner { old, new } => {
                self.old_owners.remove(new);
                self.old_owners.insert(old.clone());
            }
        };

        Ok(())
    }
}

impl Storable for TransferRegister {}
