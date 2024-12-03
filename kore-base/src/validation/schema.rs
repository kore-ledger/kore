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
    request::ValidationReq,
    validator::{Validator, ValidatorMessage},
};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ValidationSchema {
    gov_version: u64,
    creators: HashSet<KeyIdentifier>,
}

impl ValidationSchema {
    pub fn new(creators: HashSet<KeyIdentifier>, gov_version: u64) -> Self {
        ValidationSchema {
            creators,
            gov_version,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ValidationSchemaMessage {
    NetworkRequest {
        validation_req: Box<Signed<ValidationReq>>,
        info: ComunicateInfo,
    },
    UpdateValidators(HashSet<KeyIdentifier>, u64),
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
                if self.gov_version
                    < validation_req.content.proof.governance_version
                {
                    if let Err(e) = try_to_update_subject(
                        ctx,
                        validation_req.content.proof.governance_id.clone(),
                    )
                    .await
                    {
                        return Err(emit_fail(ctx, e).await);
                    }
                }

                let creator =
                    self.creators.get(&validation_req.signature.signer);
                if creator.is_none() {
                    return Err(ActorError::Functional(
                        "Sender is not a Creator".to_owned(),
                    ));
                };

                if let Err(e) = validation_req.verify() {
                    return Err(ActorError::Functional(format!(
                        "Can not verify validation request: {}.",
                        e
                    )));
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
                    Err(e) => {
                        if let ActorError::Exists(_) = e {
                            return Ok(());
                        } else {
                            return Err(emit_fail(ctx, e).await)
                        }
                    },
                };

                validator_actor
                    .tell(ValidatorMessage::NetworkRequest {
                        validation_req,
                        info,
                    })
                    .await?
            }
            ValidationSchemaMessage::UpdateValidators(
                validators,
                gov_version,
            ) => {
                self.gov_version = gov_version;
                self.creators = validators;
            }
        };
        Ok(())
    }
}
