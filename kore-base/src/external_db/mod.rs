// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::Duration;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, SystemEvent,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::error;

use crate::{
    db::Storable,
    error::Error,
    helpers::db::{ExternalDB, Querys},
};

const TARGET_EXTERNAL: &str = "Kore-ExternalDB";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeleteTypes {
    Request { id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DBManager {
    time: Duration,
    delete_req: Vec<DeleteTypes>,
}

impl DBManager {
    pub fn new(time: Duration) -> Self {
        Self {
            time,
            delete_req: vec![],
        }
    }

    async fn delete(
        &self,
        delete: DeleteTypes,
        our_ref: ActorRef<DBManager>,
        helper: ExternalDB,
    ) {
        let time = self.time;

        tokio::spawn(async move {
            tokio::time::sleep(time).await;
            match delete.clone() {
                DeleteTypes::Request { id } => {
                    if let Err(e) = helper.del_request(&id).await {
                        if let Err(e) =
                            our_ref.tell(DBManagerMessage::Error(e)).await
                        {
                            error!(TARGET_EXTERNAL, "{}", e);
                        };
                    };
                }
            };

            if let Err(e) =
                our_ref.tell(DBManagerMessage::ConfirmDelete(delete)).await
            {
                error!(TARGET_EXTERNAL, "{}", e);
            };
        });
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DBManagerMessage {
    InitDelete,
    Error(Error),
    Delete(DeleteTypes),
    ConfirmDelete(DeleteTypes),
}

impl Message for DBManagerMessage {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DBManagerEvent {
    DeleteReq(DeleteTypes),
    DeleteConfirm(DeleteTypes),
}

impl Event for DBManagerEvent {}

#[async_trait]
impl Actor for DBManager {
    type Message = DBManagerMessage;
    type Event = DBManagerEvent;
    type Response = ();

    async fn pre_start(
        &mut self,
        ctx: &mut actor::ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.init_store("ext_db", None, false, ctx).await
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<DBManager> for DBManager {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: DBManagerMessage,
        ctx: &mut actor::ActorContext<DBManager>,
    ) -> Result<(), ActorError> {
        let Some(helper): Option<ExternalDB> =
            ctx.system().get_helper("ext_db").await
        else {
            error!(TARGET_EXTERNAL, "Can not obtain ext_db helper");
            ctx.system().send_event(SystemEvent::StopSystem).await;
            let e = ActorError::NotHelper("ext_db".to_owned());
            return Err(e);
        };

        match msg {
            DBManagerMessage::InitDelete => {
                let Some(our_ref): Option<ActorRef<DBManager>> =
                    ctx.reference().await
                else {
                    error!(
                        TARGET_EXTERNAL,
                        "InitDelete, Can not obtain DBManager actor"
                    );
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    let e = ActorError::NotFound(ctx.path().clone());
                    return Err(e);
                };

                for req in self.delete_req.clone() {
                    self.delete(req, our_ref.clone(), helper.clone()).await;
                }
            }
            DBManagerMessage::Error(error) => {
                error!(
                    TARGET_EXTERNAL,
                    "Error, Problem in Subscriber: {}", error
                );
                let e = ActorError::FunctionalFail(error.to_string());
                ctx.system().send_event(SystemEvent::StopSystem).await;
                return Err(e);
            }
            DBManagerMessage::Delete(delete) => {
                self.on_event(DBManagerEvent::DeleteReq(delete.clone()), ctx)
                    .await;

                let Some(our_ref): Option<ActorRef<DBManager>> =
                    ctx.reference().await
                else {
                    error!(
                        TARGET_EXTERNAL,
                        "InitDelete, Can not obtain DBManager actor"
                    );
                    let e = ActorError::NotFound(ctx.path().clone());
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(e);
                };

                self.delete(delete, our_ref, helper).await;
            }
            DBManagerMessage::ConfirmDelete(confirm) => {
                self.on_event(DBManagerEvent::DeleteConfirm(confirm), ctx)
                    .await;
            }
        };

        Ok(())
    }

    async fn on_event(
        &mut self,
        event: DBManagerEvent,
        ctx: &mut ActorContext<DBManager>,
    ) {
        if let Err(e) = self.persist_light(&event, ctx).await {
            error!(
                TARGET_EXTERNAL,
                "OnEvent, can not persist information: {}", e
            );
            ctx.system().send_event(SystemEvent::StopSystem).await;
        };
    }
}

#[async_trait]
impl PersistentActor for DBManager {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        match event {
            DBManagerEvent::DeleteReq(delete_types) => {
                self.delete_req.push(delete_types.clone());
            }
            DBManagerEvent::DeleteConfirm(delete_types) => {
                if let Some(pos) =
                    self.delete_req.iter().position(|x| *x == *delete_types)
                {
                    let _ = self.delete_req.remove(pos);
                }
            }
        };

        Ok(())
    }
}

#[async_trait]
impl Storable for DBManager {}
