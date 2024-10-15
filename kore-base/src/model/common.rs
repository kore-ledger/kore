use actor::{Actor, ActorContext, ActorPath, ActorRef, Handler};
use identity::identifier::DigestIdentifier;

use crate::{
    model::SignTypesNode,
    subject::{
        event::{LedgerEvent, LedgerEventMessage, LedgerEventResponse},
        SubjectMetadata,
    },
    Error, Event as KoreEvent, Governance, Node, NodeMessage, NodeResponse,
    Signature, Signed, Subject, SubjectMessage, SubjectResponse,
};

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
        SubjectResponse::Error(error) => {
            return Err(Error::Actor(format!(
                "The subject encountered problems when getting governance: {}",
                error
            )));
        }
        _ => {
            return Err(Error::Actor(format!(
                "An unexpected response has been received from node actor"
            )))
        }
    }
}

pub async fn get_metadata<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<SubjectMetadata, Error>
where
    A: Actor + Handler<A>,
{
    let subject_path = ActorPath::from(format!("/user/node/{}", subject_id));
    let subject_actor: Option<ActorRef<Subject>> =
        ctx.system().get_actor(&subject_path).await;

    let response = if let Some(subject_actor) = subject_actor {
        // We ask a node
        let response =
            subject_actor.ask(SubjectMessage::GetSubjectMetadata).await;
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
        SubjectResponse::SubjectMetadata(metadata) => Ok(metadata),
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
            Err(e) => todo!(),
        }
    } else {
        todo!()
    };

    match node_response {
        NodeResponse::SignRequest(signature) => Ok(signature),
        NodeResponse::Error(_) => todo!(),
        _ => todo!(),
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
        "/user/node/{}/ledgerEvent",
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
            Err(e) => todo!(),
        }
    } else {
        todo!()
    };

    if let LedgerEventResponse::Error(e) = response {
        todo!()
    };

    Ok(())
}
