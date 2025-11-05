use crate::{
    Node, NodeMessage, NodeResponse, Signature, model::SignTypesNode,
    node::SubjectData,
};
use rush::{Actor, ActorContext, ActorError, ActorPath, ActorRef, Handler};

async fn change_node_subject<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
    new_owner: &str,
    old_owner: &str,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    if let Some(node_actor) = node_actor {
        node_actor
            .ask(NodeMessage::ChangeSubjectOwner {
                new_owner: new_owner.to_owned(),
                old_owner: old_owner.to_owned(),
                subject_id: subject_id.to_owned(),
            })
            .await?;
    } else {
        return Err(ActorError::NotFound(node_path));
    }

    Ok(())
}

pub async fn get_node_subject_data<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<Option<(SubjectData, Option<String>)>, ActorError>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    // We obtain the validator
    let node_response = if let Some(node_actor) = node_actor {
        node_actor
            .ask(NodeMessage::GetSubjectData(subject_id.to_owned()))
            .await?
    } else {
        return Err(ActorError::NotFound(node_path));
    };

    match node_response {
        NodeResponse::None => Ok(None),
        NodeResponse::SubjectData { data, new_owner } => {
            Ok(Some((data, new_owner)))
        }
        _ => Err(ActorError::UnexpectedResponse(
            node_path,
            "NodeResponse::SubjectData || NodeResponse::None".to_owned(),
        )),
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
        _ => Err(ActorError::UnexpectedResponse(
            node_path,
            "NodeResponse::SignRequest".to_owned(),
        )),
    }
}

pub async fn subject_owner<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<(bool, bool), ActorError>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<rush::ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    let response = if let Some(node_actor) = node_actor {
        node_actor
            .ask(NodeMessage::OwnerPendingSubject(subject_id.to_owned()))
            .await?
    } else {
        return Err(ActorError::NotFound(node_path));
    };

    match response {
        NodeResponse::IOwnerPending(res) => Ok(res),
        _ => Err(ActorError::UnexpectedResponse(
            node_path,
            "NodeResponse::OwnerPending".to_owned(),
        )),
    }
}

pub async fn subject_old<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<bool, ActorError>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<rush::ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    let response = if let Some(node_actor) = node_actor {
        node_actor
            .ask(NodeMessage::OldSubject(subject_id.to_owned()))
            .await?
    } else {
        return Err(ActorError::NotFound(node_path));
    };

    match response {
        NodeResponse::IOld(res) => Ok(res),
        _ => Err(ActorError::UnexpectedResponse(
            node_path,
            "NodeResponse::OwnerPending".to_owned(),
        )),
    }
}
