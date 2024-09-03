use std::collections::HashSet;

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Handler, Message,
};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};

use crate::Signed;

use super::{
    request::EvaluationReq,
    evaluator::{Evaluator, EvaluatorCommand},
};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct EvaluationSchema {
    evaluators: HashSet<KeyIdentifier>,
}

impl EvaluationSchema {
    pub fn new(evaluators: HashSet<KeyIdentifier>) -> Self {
        EvaluationSchema { evaluators }
    }
}

#[derive(Debug, Clone)]
pub enum EvaluationSchemaCommand {
    NetworkRequest {
        evaluation_req: Signed<EvaluationReq>,
        info: ComunicateInfo,
    },
    UpdateEvaluators(HashSet<KeyIdentifier>)
}

impl Message for EvaluationSchemaCommand {}

#[async_trait]
impl Actor for EvaluationSchema {
    type Event = ();
    type Message = EvaluationSchemaCommand;
    type Response = ();
}

#[async_trait]
impl Handler<EvaluationSchema> for EvaluationSchema {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: EvaluationSchemaCommand,
        ctx: &mut ActorContext<EvaluationSchema>,
    ) -> Result<(), ActorError> {
        match msg {
            EvaluationSchemaCommand::NetworkRequest {
                evaluation_req,
                info,
            } => {
                let subject_owner = self.evaluators.get(&evaluation_req.signature.signer);
                if let None = subject_owner {
                    todo!()
                };

                if let Err(e) = evaluation_req.verify() {
                    // Hay errores criptográficos
                    todo!()
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
                    Err(e) => todo!(),
                };

                if let Err(e) = evaluator_actor
                    .tell(EvaluatorCommand::NetworkRequest {
                        evaluation_req,
                        info,
                    })
                    .await
                {
                    return Err(e);
                }
            },
            EvaluationSchemaCommand::UpdateEvaluators(evaluators) => {
                self.evaluators = evaluators;
            }
        };
        Ok(())
    }
}
