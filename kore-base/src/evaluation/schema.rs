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
    evaluator::{Evaluator, EvaluatorMessage},
    request::EvaluationReq,
};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct EvaluationSchema {
    creators: HashSet<KeyIdentifier>,
}

impl EvaluationSchema {
    pub fn new(creators: HashSet<KeyIdentifier>) -> Self {
        EvaluationSchema { creators }
    }
}

#[derive(Debug, Clone)]
pub enum EvaluationSchemaMessage {
    NetworkRequest {
        evaluation_req: Signed<EvaluationReq>,
        info: ComunicateInfo,
    },
    UpdateEvaluators(HashSet<KeyIdentifier>),
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
                // TODO lo primero que hay que hacer es comprobar la versión de la governanza,
                // para comprobar que sea un creator, si estamos desactualizados, puede ser un creator
                // nuevo.
                let creator =
                    self.creators.get(&evaluation_req.signature.signer);
                if creator.is_none() {
                    todo!()
                };

                if let Err(_e) = evaluation_req.verify() {
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
                    Err(_e) => todo!(),
                };

                evaluator_actor
                    .tell(EvaluatorMessage::NetworkRequest {
                        evaluation_req,
                        info,
                    })
                    .await?
            }
            EvaluationSchemaMessage::UpdateEvaluators(creators) => {
                self.creators = creators;
            }
        };
        Ok(())
    }
}
