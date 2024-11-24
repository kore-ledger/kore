use std::collections::HashSet;

use actor::{Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Handler, SystemEvent};
use identity::identifier::{DigestIdentifier, KeyIdentifier};

use crate::{
    governance::{model::Roles, Quorum}, model::SignTypesNode, node::relationship::{
        OwnerSchema, RelationShip, RelationShipMessage, RelationShipResponse,
    }, subject::{
        event::{LedgerEvent, LedgerEventMessage, LedgerEventResponse},
        Metadata,
    }, Error, Event as KoreEvent, EventRequestType, Governance, Node, NodeMessage, NodeResponse, Signature, Signed, Subject, SubjectMessage, SubjectResponse, SubjectsTypes
};

use super::Namespace;

pub async fn get_gov<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<Governance, ActorError>
where
    A: Actor + Handler<A>,
{
    // Subject path
    let subject_path = ActorPath::from(format!("/user/node/{}", subject_id));

    // Subject actor.
    let subject_actor: Option<ActorRef<Subject>> =
        ctx.system().get_actor(&subject_path).await;

    // We obtain the actor governance
    let response = if let Some(subject_actor) = subject_actor {
        subject_actor.ask(SubjectMessage::GetGovernance).await?
    } else {
        return Err(ActorError::NotFound(subject_path));
    };

    match response {
        SubjectResponse::Governance(gov) => Ok(gov),
        SubjectResponse::Error(error) => Err(ActorError::Functional(error.to_string())),
        _ => Err(ActorError::UnexpectedMessage(subject_path, "SubjectResponse::Governance".to_owned())),
    }
}

pub async fn get_metadata<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<Metadata, ActorError>
where
    A: Actor + Handler<A>,
{
    let subject_path = ActorPath::from(format!("/user/node/{}", subject_id));
    let subject_actor: Option<ActorRef<Subject>> =
        ctx.system().get_actor(&subject_path).await;

    let response = if let Some(subject_actor) = subject_actor {
        subject_actor.ask(SubjectMessage::GetMetadata).await?
    } else {
        return Err(ActorError::NotFound(subject_path));
    };

    match response {
        SubjectResponse::Metadata(metadata) => Ok(metadata),
        _ => Err(ActorError::UnexpectedMessage(subject_path, "SubjectResponse::Metadata".to_owned())),
    }
}

pub async fn get_sign<A>(
    ctx: &mut ActorContext<A>,
    sign_type: SignTypesNode,
) -> Result<Signature, ActorError>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    // We obtain the validator
    let node_response = if let Some(node_actor) = node_actor {
        node_actor.ask(NodeMessage::SignRequest(sign_type)).await?
    } else {
        return Err(ActorError::NotFound(node_path));
    };

    match node_response {
        NodeResponse::SignRequest(signature) => Ok(signature),
        NodeResponse::Error(e) => {
            Err(ActorError::Functional(e.to_string()))
        },
        _ => {
            Err(ActorError::UnexpectedMessage(node_path, "NodeResponse::SignRequest".to_owned()))
        },
    }
}

pub async fn update_event<A>(
    ctx: &mut ActorContext<A>,
    event: Signed<KoreEvent>,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let ledger_event_path = ActorPath::from(format!(
        "/user/node/{}/ledger_event",
        event.content.subject_id
    ));
    let ledger_event_actor: Option<ActorRef<LedgerEvent>> =
        ctx.system().get_actor(&ledger_event_path).await;

    let response = if let Some(ledger_event_actor) = ledger_event_actor {
        ledger_event_actor
            .ask(LedgerEventMessage::UpdateLastEvent { event })
            .await?
    } else {
        return Err(ActorError::NotFound(ledger_event_path));
    };

    if let LedgerEventResponse::Error(e) = response {
        return Err(ActorError::Functional(e.to_string()));
    };

    Ok(())
}

pub async fn change_temp_subj<A>(
    ctx: &mut ActorContext<A>,
    subject_id: String,
    key_identifier: String,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    if let Some(node_actor) = node_actor {
        node_actor
            .tell(NodeMessage::RegisterSubject(SubjectsTypes::ChangeTemp {
                subject_id,
                key_identifier,
            }))
            .await?;
    } else {
        return Err(ActorError::NotFound(node_path));
    }
    Ok(())
}

pub async fn get_quantity<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema: String,
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
                schema,
                namespace,
            }))
            .await?
    } else {
        return Err(ActorError::NotFound(relation_path));
    };

    if let RelationShipResponse::Count(quantity) = response {
        Ok(quantity)
    } else {
        Err(ActorError::UnexpectedMessage(relation_path, "RelationShipResponse::Count".to_owned()))
    }
}

pub async fn register_relation<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema: String,
    owner: String,
    subject: String,
    namespace: String,
    max_quantity: usize,
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
                    schema,
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
        _ => Err(ActorError::UnexpectedMessage(relation_path, "RelationShipResponse::None".to_owned())),
    }
}

pub async fn delete_relation<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema: String,
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
                    schema,
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
        Err(ActorError::UnexpectedMessage(relation_path, "RelationShipResponse::None".to_owned()))
    }
}

pub fn verify_protocols_state(
    request: EventRequestType,
    eval: Option<bool>,
    approve: Option<bool>,
    approval_require: bool,
    val: bool,
) -> Result<bool, Error> {
    match request {
        EventRequestType::Create
        | EventRequestType::Transfer
        | EventRequestType::Confirm
        | EventRequestType::EOL => {
            if approve.is_some() || eval.is_some() || approval_require {
                return Err(Error::Protocols("In create, transferm, confirm and eol request, approve and eval must be None and approval require must be false".to_owned()));
            }
            Ok(val)
        }
        EventRequestType::Fact => {
            let Some(eval) = eval else { 
                return Err(Error::Protocols("In Fact even eval must be Some".to_owned()));
            };

            if approval_require {
                 let Some(approve) = approve else {
                    return Err(Error::Protocols("In Fact even if approval was required, approve must be Some".to_owned()));
                };
                Ok(eval && approve && val)
            } else {
                if approve.is_some() {
                    return Err(Error::Protocols("In Fact even if approval was not required, approve must be None".to_owned()));
                }

                Ok(val && eval)
            }
        }
    }
}


pub async fn get_signers_quorum_gov_version<A>(
    ctx: &mut ActorContext<A>,
    governance: &str,
    schema_id: &str,
    namespace: Namespace,
    role: Roles
) -> Result<(HashSet<KeyIdentifier>, Quorum, u64), ActorError> 
where 
    A: Actor + Handler<A>,
{
    let gov = get_gov(ctx, governance).await?;
    let (signers, quorum) = gov.get_quorum_and_signers(role, schema_id, namespace)?;
    Ok((signers, quorum, gov.version))
}

pub async fn emit_fail<A>(
    ctx: &mut ActorContext<A>,
    error: ActorError,
) -> ActorError
where
    A: Actor + Handler<A>,
{
    if let Err(e) = ctx.emit_fail(error.clone()).await {
        ctx.system().send_event(SystemEvent::StopSystem).await;
    };
    error
}