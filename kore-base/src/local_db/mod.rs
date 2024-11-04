use std::time::Duration;

use actor::{Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event, Handler, Message, Response};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;

use crate::{db::Storable, helpers::db::{LocalDB, Querys}, model::TimeStamp};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeleteTypes {
    Request { id: String }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DBManager {
    time: Duration,
    delete_req: Vec<DeleteTypes>
}

impl DBManager {

    pub fn new(time: Duration) -> Self {
        Self {
            time,
            delete_req: vec![],
        }
    }
    
    async fn delete(&self, ctx: &mut actor::ActorContext<DBManager>, delete: DeleteTypes, our_ref: ActorRef<DBManager>, helper: LocalDB) {
        let time = self.time.clone();

        tokio::spawn(async move {
            tokio::time::sleep(time).await;
            match delete.clone() {
                DeleteTypes::Request { id } => {
                    helper.del_request(&id).await;
                }
            };

            if let Err(e) = our_ref.tell(DBManagerMessage::ConfirmDelete(delete)).await {

            };
        });
    }

}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DBManagerMessage {
    InitDelete,
    Error(String),
    Delete(DeleteTypes),
    ConfirmDelete(DeleteTypes)
}

impl Message for DBManagerMessage {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DBManagerResponse {
    None
}

impl Response for DBManagerResponse {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DBManagerEvent {
    DeleteReq(DeleteTypes),
    DeleteConfirm(DeleteTypes)
}

impl Event for DBManagerEvent {}

#[async_trait]
impl Actor for DBManager {
    type Message = DBManagerMessage;
    type Event = DBManagerEvent;
    type Response = DBManagerResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut actor::ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.init_store("db_manager", None, false, ctx).await
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
    ) -> Result<DBManagerResponse, ActorError> {
        match msg {
            DBManagerMessage::InitDelete => {
                let Some(our_ref): Option<ActorRef<DBManager>> = ctx.system().get_actor(&ActorPath::from("/user/db_manager")).await else {
                    todo!()
                };
        
                let Some(helper): Option<LocalDB> = ctx.system().get_helper("local_db").await else {
                    todo!()
                };
        
                for req in self.delete_req.clone() {            
                    self.delete(ctx, req, our_ref.clone(), helper.clone()).await;
                }
            },
            DBManagerMessage::Error(error) => todo!(),
            DBManagerMessage::Delete(delete) => {
                self.on_event(DBManagerEvent::DeleteReq(delete.clone()), ctx).await;
                
                let Some(our_ref): Option<ActorRef<DBManager>> = ctx.system().get_actor(&ActorPath::from("/user/db_manager")).await else {
                    todo!()
                };
        
                let Some(helper): Option<LocalDB> = ctx.system().get_helper("local_db").await else {
                    todo!()
                };
                
                self.delete(ctx, delete, our_ref, helper).await;
            },
            DBManagerMessage::ConfirmDelete(confirm) => {
                self.on_event(DBManagerEvent::DeleteConfirm(confirm), ctx).await;
            }
        };
    
        Ok(DBManagerResponse::None)
    }

    async fn on_event(
        &mut self,
        event: DBManagerEvent,
        ctx: &mut ActorContext<DBManager>,
    ) {
        if let Err(_e) = self.persist(&event, ctx).await {
            // TODO Propagar error.
        };
    }
}


#[async_trait]
impl PersistentActor for DBManager {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) {
        match event {
            DBManagerEvent::DeleteReq(delete_types) => {
                self.delete_req.push(delete_types.clone());
            },
            DBManagerEvent::DeleteConfirm(delete_types) => {
                if let Some(pos) = self.delete_req.iter().position(|x| *x == *delete_types) {
                    let _ = self.delete_req.remove(pos);
                }
            },
        }
    }
}

#[async_trait]
impl Storable for DBManager {}