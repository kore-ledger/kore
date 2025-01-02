use std::collections::HashMap;

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{error, warn};

use crate::{db::Storable, governance::model::CreatorQuantity, model::common::emit_fail};

const TARGET_RELATIONSHIP: &str = "Kore-Node-RelationShip";

#[derive(Clone, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct OwnerSchema {
    pub owner: String,
    pub gov: String,
    pub schema: String,
    pub namespace: String,
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
    RegisterNewSubject {
        data: OwnerSchema,
        subject: String,
        max_quantity: CreatorQuantity,
    },
    DeleteSubject {
        data: OwnerSchema,
        subject: String,
    },
}

impl Message for RelationShipMessage {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RelationShipResponse {
    Count(usize),
    None,
}

impl Response for RelationShipResponse {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RelationShipEvent {
    NewRegister { data: OwnerSchema, subject: String },
    DeleteSubject { data: OwnerSchema, subject: String },
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
            RelationShipMessage::RegisterNewSubject {
                data,
                subject,
                max_quantity,
            } => {
                let quantity = if let Some(vec) = self.owner_subjects.get(&data)
                {
                    vec.len()
                } else {
                    0
                };

                if let CreatorQuantity::QUANTITY(max_quantity) = max_quantity {
                    if quantity >= max_quantity as usize {
                        let e = "Maximum number of subjects reached";
                        warn!(TARGET_RELATIONSHIP, "RegisterNewSubject, {}", e);
                        return Err(ActorError::Functional(
                            e.to_owned(),
                        ));
                    }
                }

                self.on_event(
                    RelationShipEvent::NewRegister { data, subject },
                    ctx,
                )
                .await;
                Ok(RelationShipResponse::None)

            }
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
        if let Err(e) = self.persist_light(&event, ctx).await {
            error!(TARGET_RELATIONSHIP, "OnEvent, can not persist information: {}", e);
            emit_fail(ctx, e).await;
        };
    }
}

#[async_trait]
impl PersistentActor for RelationShip {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        match event {
            RelationShipEvent::NewRegister { data, subject } => {
                self.owner_subjects
                    .entry(data.clone())
                    .or_default()
                    .push(subject.clone());
            }
            RelationShipEvent::DeleteSubject { data, subject } => {
                self.owner_subjects.entry(data.clone()).and_modify(|vec| {
                    if let Some(pos) =
                        vec.iter().position(|x| x.clone() == subject.clone())
                    {
                        vec.remove(pos);
                    } else {
                        error!(TARGET_RELATIONSHIP, "An attempt was made to delete a subject that was not registered");
                    };
                });
            }
        };

        Ok(())
    }
}

#[async_trait]
impl Storable for RelationShip {}
