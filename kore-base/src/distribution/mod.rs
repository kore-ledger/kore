use std::collections::HashSet;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Handler,
    Message,
};
use async_trait::async_trait;
use distributor::{Distributor, DistributorMessage};
use identity::identifier::{DigestIdentifier, KeyIdentifier};

use crate::{
    governance::model::Roles,
    model::{common::{emit_fail, get_gov, get_metadata}, event::Ledger},
    request::manager::{RequestManager, RequestManagerMessage},
    subject::Metadata,
    Error, Event as KoreEvent, Governance, Signed, Subject, SubjectMessage,
    SubjectResponse,
};

pub mod distributor;

#[derive(Default)]
pub struct Distribution {
    witnesses: HashSet<KeyIdentifier>,
    node_key: KeyIdentifier,
    request_id: String,
}

impl Distribution {
    pub fn new(node_key: KeyIdentifier) -> Self {
        Distribution {
            node_key,
            ..Default::default()
        }
    }

    fn check_witness(&mut self, witness: KeyIdentifier) -> bool {
        self.witnesses.remove(&witness)
    }

    async fn create_distributors(
        &self,
        ctx: &mut ActorContext<Distribution>,
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>,
        signer: KeyIdentifier,
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
            Err(_e) => return Err(_e),
        };

        let our_key = self.node_key.clone();

        if signer != our_key {
            distributor_actor
                .tell(DistributorMessage::NetworkDistribution {
                    ledger,
                    event,
                    node_key: signer,
                    our_key,
                })
                .await?
        }

        Ok(())
    }

    async fn end_request(
        &self,
        ctx: &mut ActorContext<Distribution>,
    ) -> Result<(), ActorError> {
        let req_path =
            ActorPath::from(format!("/user/request/{}", self.request_id));
        let req_actor: Option<ActorRef<RequestManager>> =
            ctx.system().get_actor(&req_path).await;

        if let Some(req_actor) = req_actor {
            req_actor.tell(RequestManagerMessage::FinishRequest).await?;
        } else {
            return Err(ActorError::NotFound(req_path));
        };

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
        _sender: ActorPath,
        msg: DistributionMessage,
        ctx: &mut ActorContext<Distribution>,
    ) -> Result<(), ActorError> {
        match msg {
            DistributionMessage::Create {
                request_id,
                event,
                ledger,
            } => {
                self.request_id = request_id;
                let subject_id = ledger.content.subject_id.clone();
                // TODO, a lo mejor en el comando de creación se pueden incluir el namespace y el schema
                let governance = match get_gov(ctx, &subject_id.to_string()).await {
                    Ok(gov) => gov,
                    Err(e) => return Err(emit_fail(ctx, e).await),
                };

                let metadata = match get_metadata(ctx, &subject_id.to_string()).await {
                    Ok(metadata) => metadata,
                    Err(e) => return Err(emit_fail(ctx, e).await),
                };

                let witnesses = if metadata.schema_id == "governance" {
                    governance.members_to_key_identifier()
                } else {
                    governance
                        .get_signers(
                            Roles::WITNESS,
                            &metadata.schema_id,
                            metadata.namespace,
                        )
                        .0
                };

                if witnesses.len() == 1 && witnesses.contains(&self.node_key) {
                    if let Err(e) = self.end_request(ctx).await {
                        return Err(emit_fail(ctx, e).await);
                    };
                }

                self.witnesses = witnesses.clone();

                for witness in witnesses {
                    self.create_distributors(
                        ctx,
                        event.clone(),
                        ledger.clone(),
                        witness,
                    )
                    .await?
                }
            }
            DistributionMessage::Response { sender } => {
                if self.check_witness(sender) && self.witnesses.is_empty() {
                    if let Err(e) = self.end_request(ctx).await {
                        return Err(emit_fail(ctx, e).await)
                    };
                }
            }
        }

        Ok(())
    }
}
