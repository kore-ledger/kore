use std::collections::HashSet;

use actor::{Actor, ActorContext, ActorPath, ActorRef, Handler, SystemEvent};
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
) -> Result<Governance, Error>
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
        // We ask a governance
        let response = subject_actor.ask(SubjectMessage::GetGovernance).await;
        match response {
            Ok(response) => response,
            Err(e) => {
                return Err(Error::Actor(format!(
                    "Error when asking a Subject {}",
                    e
                )));
            }
        }
    } else {
        return Err(Error::Actor(format!(
            "The subject actor was not found in the expected path {}",
            subject_path
        )));
    };

    match response {
        SubjectResponse::Governance(gov) => Ok(gov),
        SubjectResponse::Error(error) => Err(Error::Actor(format!(
            "The subject encountered problems when getting governance: {}",
            error
        ))),
        _ => Err(Error::Actor(
            "An unexpected response has been received from node actor"
                .to_owned(),
        )),
    }
}

pub async fn get_metadata<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<Metadata, Error>
where
    A: Actor + Handler<A>,
{
    let subject_path = ActorPath::from(format!("/user/node/{}", subject_id));
    let subject_actor: Option<ActorRef<Subject>> =
        ctx.system().get_actor(&subject_path).await;

    let response = if let Some(subject_actor) = subject_actor {
        // We ask a node
        let response = subject_actor.ask(SubjectMessage::GetMetadata).await;
        match response {
            Ok(response) => response,
            Err(e) => {
                return Err(Error::Actor(format!(
                    "Error when asking a subject {}",
                    e
                )));
            }
        }
    } else {
        return Err(Error::Actor(format!(
            "The node actor was not found in the expected path {}",
            subject_path
        )));
    };

    match response {
        SubjectResponse::Metadata(metadata) => Ok(metadata),
        _ => Err(Error::Actor(
            "An unexpected response has been received from subject actor"
                .to_owned(),
        )),
    }
}

pub async fn get_sign<A>(
    ctx: &mut ActorContext<A>,
    sign_type: SignTypesNode,
) -> Result<Signature, Error>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    // We obtain the validator
    let node_response = if let Some(node_actor) = node_actor {
        match node_actor.ask(NodeMessage::SignRequest(sign_type)).await {
            Ok(response) => response,
            Err(e) => {
                return Err(Error::Actor(e.to_string()));
            },
        }
    } else {
        return Err(Error::Actor("Can not get Node actor".to_owned()));
    };

    match node_response {
        NodeResponse::SignRequest(signature) => Ok(signature),
        NodeResponse::Error(e) => {
            Err(e)
        },
        _ => {
            Err(Error::Node("Invalid node response".to_owned()))
        },
    }
}

pub async fn update_event<A>(
    ctx: &mut ActorContext<A>,
    event: Signed<KoreEvent>,
) -> Result<(), Error>
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
        match ledger_event_actor
            .ask(LedgerEventMessage::UpdateLastEvent { event })
            .await
        {
            Ok(res) => res,
            Err(_e) => todo!(),
        }
    } else {
        todo!()
    };

    if let LedgerEventResponse::Error(e) = response {
        todo!()
    };

    Ok(())
}

pub async fn change_temp_subj<A>(
    ctx: &mut ActorContext<A>,
    subject_id: String,
    key_identifier: String,
) -> Result<(), Error>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    if let Some(node_actor) = node_actor {
        if let Err(_e) = node_actor
            .tell(NodeMessage::RegisterSubject(SubjectsTypes::ChangeTemp {
                subject_id,
                key_identifier,
            }))
            .await
        {
            todo!()
        }
    } else {
        todo!()
    }
    Ok(())
}

pub async fn get_quantity<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema: String,
    owner: String,
    namespace: String,
) -> Result<usize, Error>
where
    A: Actor + Handler<A>,
{
    let relation_path = ActorPath::from("/user/node/relation_ship");
    let relation_actor: Option<ActorRef<RelationShip>> =
        ctx.system().get_actor(&relation_path).await;

    let response = if let Some(relation_actor) = relation_actor {
        let Ok(result) = relation_actor
            .ask(RelationShipMessage::GetSubjectsCount(OwnerSchema {
                owner,
                gov,
                schema,
                namespace,
            }))
            .await
        else {
            todo!()
        };
        result
    } else {
        todo!()
    };

    if let RelationShipResponse::Count(quantity) = response {
        return Ok(quantity);
    } else {
        todo!()
    };
}

pub async fn register_relation<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema: String,
    owner: String,
    subject: String,
    namespace: String,
    max_quantity: usize,
) -> Result<(), Error>
where
    A: Actor + Handler<A>,
{
    let relation_path = ActorPath::from("/user/node/relation_ship");
    let relation_actor: Option<ActorRef<RelationShip>> =
        ctx.system().get_actor(&relation_path).await;

    let response = if let Some(relation_actor) = relation_actor {
        let Ok(result) = relation_actor
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
            .await
        else {
            todo!()
        };
        result
    } else {
        todo!()
    };

    match response {
        RelationShipResponse::None => Ok(()),
        RelationShipResponse::Error(e) => Err(e),
        _ => todo!(),
    }
}

pub async fn delete_relation<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema: String,
    owner: String,
    subject: String,
    namespace: String,
) -> Result<(), Error>
where
    A: Actor + Handler<A>,
{
    let relation_path = ActorPath::from("/user/node/relation_ship");
    let relation_actor: Option<ActorRef<RelationShip>> =
        ctx.system().get_actor(&relation_path).await;

    let response = if let Some(relation_actor) = relation_actor {
        let Ok(result) = relation_actor
            .ask(RelationShipMessage::DeleteSubject {
                data: OwnerSchema {
                    owner,
                    gov,
                    schema,
                    namespace,
                },
                subject,
            })
            .await
        else {
            todo!()
        };
        result
    } else {
        todo!()
    };

    if let RelationShipResponse::None = response {
        return Ok(());
    } else {
        todo!()
    };
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
                todo!()
            }
            Ok(val)
        }
        EventRequestType::Fact => {
            let eval = if let Some(eval) = eval { eval } else { todo!() };

            if approval_require {
                let approve = if let Some(approve) = approve {
                    approve
                } else {
                    todo!()
                };
                Ok(eval && approve && val)
            } else {
                if let Some(_approve) = approve {
                    todo!()
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
) -> Result<(HashSet<KeyIdentifier>, Quorum, u64), Error> 
where 
    A: Actor + Handler<A>,
{
    let gov = get_gov(ctx, governance).await?;
    match gov.get_quorum_and_signers(role, schema_id, namespace) {
        Ok((signers, quorum)) => Ok((signers, quorum, gov.version)),
        Err(error) => Err(Error::Actor(format!("The governance encountered problems when getting signers and quorum: {}",error)))
    }
}

