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
    request::ValidationReq,
    validator::{Validator, ValidatorMessage},
};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ValidationSchema {
    creators: HashSet<KeyIdentifier>,
}

impl ValidationSchema {
    pub fn new(creators: HashSet<KeyIdentifier>) -> Self {
        ValidationSchema { creators }
    }
}

#[derive(Debug, Clone)]
pub enum ValidationSchemaMessage {
    NetworkRequest {
        validation_req: Box<Signed<ValidationReq>>,
        info: ComunicateInfo,
    },
    UpdateValidators(HashSet<KeyIdentifier>),
}

impl Message for ValidationSchemaMessage {}

#[async_trait]
impl Actor for ValidationSchema {
    type Event = ();
    type Message = ValidationSchemaMessage;
    type Response = ();
}

#[async_trait]
impl Handler<ValidationSchema> for ValidationSchema {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: ValidationSchemaMessage,
        ctx: &mut ActorContext<ValidationSchema>,
    ) -> Result<(), ActorError> {
        match msg {
            ValidationSchemaMessage::NetworkRequest {
                validation_req,
                info,
            } => {
                // TODO lo primero que hay que hacer es comprobar la versión de la governanza,
                // para comprobar que sea un creator.
                let creator =
                    self.creators.get(&validation_req.signature.signer);
                if creator.is_none() {
                    todo!()
                };

                if let Err(_e) = validation_req.verify() {
                    // Hay errores criptográficos
                    todo!()
                }

                let child = ctx
                    .create_child(
                        &format!("{}", validation_req.signature.signer),
                        Validator::new(
                            info.request_id.clone(),
                            validation_req.signature.signer.clone(),
                        ),
                    )
                    .await;

                let validator_actor = match child {
                    Ok(child) => child,
                    Err(_e) => todo!(),
                };

                validator_actor
                    .tell(ValidatorMessage::NetworkRequest {
                        validation_req,
                        info,
                    })
                    .await?
            }
            ValidationSchemaMessage::UpdateValidators(validators) => {
                self.creators = validators;
            }
        };
        Ok(())
    }
}
