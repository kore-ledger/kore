use actor::{Actor, ActorContext, ActorPath, ActorRef, Handler};
use identity::identifier::DigestIdentifier;

use crate::{
    subject::SubjectMetadata, Error, Governance, Subject, SubjectMessage,
    SubjectResponse,
};

pub async fn get_gov<A>(
    ctx: &mut ActorContext<A>,
    governance_id: DigestIdentifier,
) -> Result<Governance, Error>
where
    A: Actor + Handler<A>,
{
    // Governance path
    let governance_path =
        ActorPath::from(format!("/user/node/{}", governance_id));

    // Governance actor.
    let governance_actor: Option<ActorRef<Subject>> =
        ctx.system().get_actor(&governance_path).await;

    // We obtain the actor governance
    let response = if let Some(governance_actor) = governance_actor {
        // We ask a governance
        let response =
            governance_actor.ask(SubjectMessage::GetGovernance).await;
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
            "The governance actor was not found in the expected path {}",
            governance_path
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
    subject_id: DigestIdentifier,
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
