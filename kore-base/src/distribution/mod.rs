use std::collections::HashSet;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Handler,
    Message,
};
use async_trait::async_trait;
use distributor::{Distributor, DistributorMessage};
use identity::identifier::{DigestIdentifier, KeyIdentifier};

use crate::{
    governance::{self, model::Roles},
    model::event::Ledger,
    request::manager::{RequestManager, RequestManagerMessage},
    subject::SubjectMetadata,
    Error, Event as KoreEvent, Governance, Signed, Subject, SubjectMessage,
    SubjectResponse,
};

pub mod distributor;

pub struct Distribution {
    witnesses: HashSet<KeyIdentifier>,
    node_key: KeyIdentifier,
    request_id: String,
}

impl Distribution {
    fn check_witness(&mut self, witness: KeyIdentifier) -> bool {
        self.witnesses.remove(&witness)
    }

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
            governance_actor.ask(SubjectMessage::GetGovernance).await;
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
            .ask(SubjectMessage::GetSubjectMetadata)
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
        ledger: Signed<Ledger>,
        signer: KeyIdentifier,
        gov_version: u64,
    ) -> Result<(), ActorError> {
        let child = ctx
            .create_child(
                &format!("{}", signer),
                Distributor {
                    node: signer.clone(),
                },
            )
            .await;
        let distributor_actor = match child {
            Ok(child) => child,
            Err(e) => return Err(e),
        };

        let our_key = self.node_key.clone();

        if signer != our_key {
            distributor_actor
                .tell(DistributorMessage::NetworkDistribution {
                    gov_version,
                    ledger,
                    event,
                    node_key: signer,
                    our_key,
                })
                .await?
        }

        Ok(())
    }
}

#[async_trait]
impl Actor for Distribution {
    type Event = ();
    type Message = DistributionMessage;
    type Response = ();
}

#[derive(Debug, Clone)]
pub enum DistributionMessage {
    Create {
        request_id: String,
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>,
    },
    Response {
        sender: KeyIdentifier,
    },
}

impl Message for DistributionMessage {}

#[async_trait]
impl Handler<Distribution> for Distribution {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: DistributionMessage,
        ctx: &mut ActorContext<Distribution>,
    ) -> Result<(), ActorError> {
        match msg {
            DistributionMessage::Create {
                request_id,
                event,
                ledger,
            } => {
                let subject_id = ledger.content.subject_id.clone();
                // TODO, a lo mejor en el comando de creación se pueden incluir el namespace y el schema
                let (governance, metadata) =
                    match self.get_gov_metadata(ctx, subject_id).await {
                        Ok(gov) => gov,
                        Err(e) => todo!(),
                    };

                self.request_id = request_id.to_string();

                let witnesses = if metadata.schema_id == "governance" {
                    governance.members_to_key_identifier()
                } else {
                    governance.get_signers(
                        Roles::WITNESS,
                        &metadata.schema_id,
                        metadata.namespace,
                    )
                };

                for witness in witnesses {
                    self.create_distributors(
                        ctx,
                        event.clone(),
                        ledger.clone(),
                        witness,
                        governance.version,
                    )
                    .await?
                }
            }
            DistributionMessage::Response { sender } => {
                if self.check_witness(sender) {
                    if self.witnesses.is_empty() {
                        let req_path = ActorPath::from(format!(
                            "/user/request/{}",
                            self.request_id
                        ));
                        let req_actor: Option<ActorRef<RequestManager>> =
                            ctx.system().get_actor(&req_path).await;

                        if let Some(req_actor) = req_actor {
                            if let Err(e) = req_actor
                                .tell(RequestManagerMessage::FinishRequest)
                                .await
                            {
                                todo!()
                            }
                        } else {
                            todo!()
                        };
                    }
                }
            }
        }

        Ok(())
    }
}
