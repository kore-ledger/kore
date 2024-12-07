use std::collections::HashSet;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, ChildAction, Error as ActorError,
    Handler, Message,
};
use async_trait::async_trait;
use distributor::{Distributor, DistributorMessage};
use identity::identifier::KeyIdentifier;
use tracing::error;

use crate::{
    governance::model::Roles,
    model::{
        common::{emit_fail, get_gov, get_metadata},
        event::Ledger,
    },
    request::manager::{RequestManager, RequestManagerMessage},
    Event as KoreEvent, Signed,
};

pub mod distributor;

const TARGET_DISTRIBUTION: &str = "Kore-Distribution";

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
        schema_id: &str,
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
                    ledger,
                    event,
                    node_key: signer,
                    our_key,
                    schema_id: schema_id.to_string(),
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
                let governance =
                    match get_gov(ctx, &subject_id.to_string()).await {
                        Ok(gov) => gov,
                        Err(e) => {
                            error!(TARGET_DISTRIBUTION, "Create, can not get governance: {}", e);
                            return Err(emit_fail(ctx, e).await)
                        },
                    };

                let metadata =
                    match get_metadata(ctx, &subject_id.to_string()).await {
                        Ok(metadata) => metadata,
                        Err(e) => {
                            error!(TARGET_DISTRIBUTION, "Create, can not get metadata: {}", e);
                            return Err(emit_fail(ctx, e).await)
                        },
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
                        error!(TARGET_DISTRIBUTION, "Create, can not end distribution: {}", e);
                        return Err(emit_fail(ctx, e).await);
                    };
                    return Ok(());
                }

                self.witnesses.clone_from(&witnesses);

                for witness in witnesses {
                    self.create_distributors(
                        ctx,
                        event.clone(),
                        ledger.clone(),
                        witness,
                        &metadata.schema_id,
                    )
                    .await?
                }
            }
            DistributionMessage::Response { sender } => {
                if self.check_witness(sender) && self.witnesses.is_empty() {
                    if let Err(e) = self.end_request(ctx).await {
                        error!(TARGET_DISTRIBUTION, "Response, can not end distribution: {}", e);
                        return Err(emit_fail(ctx, e).await);
                    };
                }
            }
        }

        Ok(())
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Distribution>,
    ) -> ChildAction {
        error!(TARGET_DISTRIBUTION, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}
