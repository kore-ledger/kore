use crate::{
    governance::model::CreatorQuantity,
    node::relationship::{
        OwnerSchema, RelationShip, RelationShipMessage, RelationShipResponse,
    },
};
use rush::{Actor, ActorContext, ActorError, ActorPath, ActorRef, Handler};

pub async fn get_quantity<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema_id: String,
    owner: String,
    namespace: String,
) -> Result<usize, ActorError>
where
    A: Actor + Handler<A>,
{
    let relation_path = ActorPath::from("/user/node/relation_ship");
    let relation_actor: Option<ActorRef<RelationShip>> =
        ctx.system().get_actor(&relation_path).await;

    let response = if let Some(relation_actor) = relation_actor {
        relation_actor
            .ask(RelationShipMessage::GetSubjectsCount(OwnerSchema {
                owner,
                gov,
                schema_id,
                namespace,
            }))
            .await?
    } else {
        return Err(ActorError::NotFound(relation_path));
    };

    if let RelationShipResponse::Count(quantity) = response {
        Ok(quantity)
    } else {
        Err(ActorError::UnexpectedResponse(
            relation_path,
            "RelationShipResponse::Count".to_owned(),
        ))
    }
}

pub async fn register_relation<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema_id: String,
    owner: String,
    subject: String,
    namespace: String,
    max_quantity: CreatorQuantity,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let relation_path = ActorPath::from("/user/node/relation_ship");
    let relation_actor: Option<ActorRef<RelationShip>> =
        ctx.system().get_actor(&relation_path).await;

    let response = if let Some(relation_actor) = relation_actor {
        relation_actor
            .ask(RelationShipMessage::RegisterNewSubject {
                data: OwnerSchema {
                    owner,
                    gov,
                    schema_id,
                    namespace,
                },
                subject,
                max_quantity,
            })
            .await?
    } else {
        return Err(ActorError::NotFound(relation_path));
    };

    match response {
        RelationShipResponse::None => Ok(()),
        _ => Err(ActorError::UnexpectedResponse(
            relation_path,
            "RelationShipResponse::None".to_owned(),
        )),
    }
}

pub async fn delete_relation<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema_id: String,
    owner: String,
    subject: String,
    namespace: String,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let relation_path = ActorPath::from("/user/node/relation_ship");
    let relation_actor: Option<ActorRef<RelationShip>> =
        ctx.system().get_actor(&relation_path).await;

    let response = if let Some(relation_actor) = relation_actor {
        relation_actor
            .ask(RelationShipMessage::DeleteSubject {
                data: OwnerSchema {
                    owner,
                    gov,
                    schema_id,
                    namespace,
                },
                subject,
            })
            .await?
    } else {
        return Err(ActorError::NotFound(relation_path));
    };

    if let RelationShipResponse::None = response {
        Ok(())
    } else {
        Err(ActorError::UnexpectedResponse(
            relation_path,
            "RelationShipResponse::None".to_owned(),
        ))
    }
}
