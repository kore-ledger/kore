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
    validator::{Validator, ValidatorCommand},
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
pub enum ValidationSchemaCommand {
    NetworkRequest {
        validation_req: Signed<ValidationReq>,
        info: ComunicateInfo,
    },
    UpdateValidators(HashSet<KeyIdentifier>),
}

impl Message for ValidationSchemaCommand {}

#[async_trait]
impl Actor for ValidationSchema {
    type Event = ();
    type Message = ValidationSchemaCommand;
    type Response = ();
}

#[async_trait]
impl Handler<ValidationSchema> for ValidationSchema {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: ValidationSchemaCommand,
        ctx: &mut ActorContext<ValidationSchema>,
    ) -> Result<(), ActorError> {
        match msg {
            ValidationSchemaCommand::NetworkRequest {
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

                if let Err(e) = validation_req.verify() {
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
                    Err(e) => todo!(),
                };

                validator_actor
                    .tell(ValidatorCommand::NetworkRequest {
                        validation_req,
                        info,
                    })
                    .await?
            }
            ValidationSchemaCommand::UpdateValidators(validators) => {
                self.creators = validators;
            }
        };
        Ok(())
    }
}
