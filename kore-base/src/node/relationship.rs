use std::{collections::HashMap, time::Duration};

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;

use crate::{db::Storable, error::Error};

#[derive(Clone, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct OwnerSchema {
    pub owner: String,
    pub gov: String,
    pub schema: String,
    pub namespace: String
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeleteTypes {
    Request { id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RelationShip {
    owner_subjects: HashMap<OwnerSchema, Vec<String>>,
}

impl RelationShip {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RelationShipMessage {
    GetSubjectsCount(OwnerSchema),
    RegisterNewSubject { data: OwnerSchema, subject: String, max_quantity: usize },
    DeleteSubject { data: OwnerSchema, subject: String }
}

impl Message for RelationShipMessage {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RelationShipResponse {
    Count(usize),
    None,
    Error(Error)
}

impl Response for RelationShipResponse {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RelationShipEvent {
    NewRegister { data: OwnerSchema, subject: String },
    DeleteSubject { data: OwnerSchema, subject: String }
}

impl Event for RelationShipEvent {}

#[async_trait]
impl Actor for RelationShip {
    type Message = RelationShipMessage;
    type Event = RelationShipEvent;
    type Response = RelationShipResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut actor::ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.init_store("relation_ship", None, false, ctx).await
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<RelationShip> for RelationShip {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: RelationShipMessage,
        ctx: &mut actor::ActorContext<RelationShip>,
    ) -> Result<RelationShipResponse, ActorError> {
        match msg {
            RelationShipMessage::GetSubjectsCount(owner_schema) => {
                if let Some(vec) = self.owner_subjects.get(&owner_schema) {
                    Ok(RelationShipResponse::Count(vec.len()))
                } else {
                    Ok(RelationShipResponse::Count(0))
                }
            }
            RelationShipMessage::RegisterNewSubject { data, subject, max_quantity } => {
                let quantity = if let Some(vec) = self.owner_subjects.get(&data) {
                    vec.len()
                } else {
                    0
                };

                if quantity < max_quantity {
                    self.on_event(
                        RelationShipEvent::NewRegister { data, subject },
                        ctx,
                    )
                    .await;
                    Ok(RelationShipResponse::None)
                } else {
                    Ok(RelationShipResponse::Error(Error::RelationShip("Maximum number of subjects reached".to_owned())))
                }
            },
            RelationShipMessage::DeleteSubject { data, subject } => {
                self.on_event(
                    RelationShipEvent::DeleteSubject { data, subject },
                    ctx,
                )
                .await;
                Ok(RelationShipResponse::None)
            }
        }
    }

    async fn on_event(
        &mut self,
        event: RelationShipEvent,
        ctx: &mut ActorContext<RelationShip>,
    ) {
        if let Err(_e) = self.persist(&event, ctx).await {
            // TODO Propagar error.
        };
    }
}

#[async_trait]
impl PersistentActor for RelationShip {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) {
        match event {
            RelationShipEvent::NewRegister { data, subject } => {
                self.owner_subjects
                    .entry(data.clone())
                    .or_insert_with(Vec::new)
                    .push(subject.clone());
            },
            RelationShipEvent::DeleteSubject { data, subject } => {
                self.owner_subjects
                    .entry(data.clone())
                    .and_modify(|vec| {
                        if let Some(pos) = vec.iter().position(|x| x.clone() == subject.clone()) {
                            vec.remove(pos);
                        } else {
                            todo!()
                        };
                    });
            }
        }
    }
}

#[async_trait]
impl Storable for RelationShip {}
