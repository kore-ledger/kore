use std::collections::HashSet;

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Handler, Message,
};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};

use crate::{
    model::common::{emit_fail, try_to_update_subject},
    Signed,
};

use super::{
    evaluator::{Evaluator, EvaluatorMessage},
    request::EvaluationReq,
};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct EvaluationSchema {
    gov_version: u64,
    creators: HashSet<KeyIdentifier>,
}

impl EvaluationSchema {
    pub fn new(creators: HashSet<KeyIdentifier>, gov_version: u64) -> Self {
        EvaluationSchema {
            creators,
            gov_version,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EvaluationSchemaMessage {
    NetworkRequest {
        evaluation_req: Signed<EvaluationReq>,
        info: ComunicateInfo,
    },
    UpdateEvaluators(HashSet<KeyIdentifier>, u64),
}

impl Message for EvaluationSchemaMessage {}

#[async_trait]
impl Actor for EvaluationSchema {
    type Event = ();
    type Message = EvaluationSchemaMessage;
    type Response = ();
}

#[async_trait]
impl Handler<EvaluationSchema> for EvaluationSchema {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: EvaluationSchemaMessage,
        ctx: &mut ActorContext<EvaluationSchema>,
    ) -> Result<(), ActorError> {
        match msg {
            EvaluationSchemaMessage::NetworkRequest {
                evaluation_req,
                info,
            } => {
                if self.gov_version < evaluation_req.content.gov_version {
                    if let Err(e) = try_to_update_subject(
                        ctx,
                        evaluation_req.content.context.governance_id.clone(),
                    )
                    .await
                    {
                        return Err(emit_fail(ctx, e).await);
                    }
                }

                let creator =
                    self.creators.get(&evaluation_req.signature.signer);
                if creator.is_none() {
                    return Err(ActorError::Functional(
                        "Sender is not a Creator".to_owned(),
                    ));
                };

                if let Err(e) = evaluation_req.verify() {
                    return Err(ActorError::Functional(format!(
                        "Can not verify evaluation request: {}.",
                        e
                    )));
                }

                let child = ctx
                    .create_child(
                        &format!("{}", evaluation_req.signature.signer),
                        Evaluator::new(
                            info.request_id.clone(),
                            evaluation_req.signature.signer.clone(),
                        ),
                    )
                    .await;

                let evaluator_actor = match child {
                    Ok(child) => child,
                    Err(e) => return Err(emit_fail(ctx, e).await),
                };

                evaluator_actor
                    .tell(EvaluatorMessage::NetworkRequest {
                        evaluation_req,
                        info,
                    })
                    .await?
            }
            EvaluationSchemaMessage::UpdateEvaluators(
                creators,
                gov_version,
            ) => {
                self.creators = creators;
                self.gov_version = gov_version;
            }
        };
        Ok(())
    }
}
