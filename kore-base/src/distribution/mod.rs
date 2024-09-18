use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Handler,
    Message,
};
use async_trait::async_trait;
use distributor::{Distributor, DistributorCommand};
use identity::{identifier::{DigestIdentifier, KeyIdentifier}, keys::KeyPair};
use tracing::event;

use crate::{
    governance::model::Roles, subject::SubjectMetadata, Error,
    Event as KoreEvent, Governance, Signed, Subject, SubjectCommand,
    SubjectResponse,
};

pub mod distributor;

pub struct Distribution {
    node_key: KeyIdentifier,
}

impl Distribution {
    async fn get_gov_metadata(
        &self,
        ctx: &mut ActorContext<Distribution>,
        subject_id: DigestIdentifier,
    ) -> Result<(Governance, SubjectMetadata), Error> {
        // Governance path
        let governance_path =
            ActorPath::from(format!("/user/node/{}", subject_id));
        // Governance actor.
        let governance_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&governance_path).await;

        // We obtain the actor governance
        let governance_actor = if let Some(governance_actor) = governance_actor
        {
            governance_actor
        } else {
            return Err(Error::Actor(format!(
                "The governance actor was not found in the expected path {}",
                governance_path
            )));
        };

        // We ask a governance
        let response =
            governance_actor.ask(SubjectCommand::GetGovernance).await;
        let response = match response {
            Ok(response) => response,
            Err(e) => {
                return Err(Error::Actor(format!(
                    "Error when asking a Subject {}",
                    e
                )));
            }
        };

        let gov =
            match response {
                SubjectResponse::Governance(gov) => gov,
                SubjectResponse::Error(error) => {
                    return Err(Error::Actor(format!(
                "The subject encountered problems when getting governance: {}",
                error
            )))
                }
                _ => return Err(Error::Actor(
                    "An unexpected response has been received from node actor"
                        .to_owned(),
                )),
            };

        let response = governance_actor
            .ask(SubjectCommand::GetSubjectMetadata)
            .await;
        let response = match response {
            Ok(response) => response,
            Err(e) => {
                return Err(Error::Actor(format!(
                    "Error when asking a Subject {}",
                    e
                )));
            }
        };

        let metadata =
            match response {
                SubjectResponse::SubjectMetadata(metadata) => metadata,
                _ => return Err(Error::Actor(
                    "An unexpected response has been received from node actor"
                        .to_owned(),
                )),
            };

        Ok((gov, metadata))
    }

    async fn create_distributors(
        &self,
        ctx: &mut ActorContext<Distribution>,
        event: Signed<KoreEvent>,
        signer: KeyIdentifier,
        subject_keys: Option<KeyPair>
    ) -> Result<(), ActorError> {
        let child = ctx
            .create_child(&format!("{}", signer), Distributor{})
            .await;
        let distributor_actor = match child {
            Ok(child) => child,
            Err(e) => return Err(e),
        };

        let our_key = self.node_key.clone();

        if signer != our_key {
            distributor_actor
                .tell(DistributorCommand::NetworkDistribution { event, subject_keys, node_key: signer, our_key } )
                .await?
        }

        Ok(())
    }
}

#[async_trait]
impl Actor for Distribution {
    type Event = ();
    type Message = DistributionCommand;
    type Response = ();
}

#[derive(Debug, Clone)]
pub struct DistributionCommand(Signed<KoreEvent>);

impl Message for DistributionCommand {}

#[async_trait]
impl Handler<Distribution> for Distribution {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: DistributionCommand,
        ctx: &mut ActorContext<Distribution>,
    ) -> Result<(), ActorError> {

        let subject_id = msg.0.content.subject_id.clone();
        // TODO, a lo mejor en el comando de creación se pueden incluir el namespace y el schema
        let (governance, metadata) =
            match self.get_gov_metadata(ctx, subject_id).await {
                Ok(gov) => gov,
                Err(e) => todo!(),
            };

        let witnesses = if metadata.schema_id == "governance" {
            governance.members_to_key_identifier()
        } else {
            governance.get_signers(
                Roles::WITNESS,
                &metadata.schema_id,
                metadata.namespace,
            )
        };

        // Si es un evento de creación necesitamos las claves del sujeto para crearlo en el otro nodo.
        let subject_keys = if msg.0.content.sn == 0 {
            Some(metadata.keys)
        } else {
            None
        };

        for witness in witnesses {
           self.create_distributors(ctx, msg.0.clone(), witness, subject_keys.clone()) .await?
        }
        Ok(())
    }
}
