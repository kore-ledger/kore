// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{request::ValidationReq,};


use async_trait::async_trait;
use identity::identifier::{
    derive::digest::DigestDerivator, key_identifier, DigestIdentifier, KeyIdentifier
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event, Handler, Message, Response, Retry, RetryStrategy, Strategy
};

use tracing::{debug, error};

/// A struct representing a validator actor.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    finish: bool
}

impl Validator {
    async fn validation_event() {

    }

    fn is_finished(&self) -> bool {
        self.finish
    }

    fn set_finished(&mut self, value: bool) {
        self.finish = value;
    }
}

#[derive(Debug, Clone)]
pub enum ValidatorCommand {
    LocalValidation((ValidationReq, KeyIdentifier)),
    NetworkValidation((ValidationReq, KeyIdentifier)),
    NetworkResponse
}

impl Message for ValidatorCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidatorEvent {
    AllTryHaveBeenMade,
    ReTry
}

impl Event for ValidatorEvent {}

#[derive(Debug, Clone)]
pub enum ValidatorResponse {
    None,
}

impl Response for ValidatorResponse {}

#[async_trait]
impl Actor for Validator {
    type Event = ValidatorEvent;
    type Message = ValidatorCommand;
    type Response = ValidatorResponse;
}

#[async_trait]
impl Handler<Validator> for Validator {
    async fn handle_message(
        &mut self,
        msg: ValidatorCommand,
        ctx: &mut ActorContext<Validator>,
    ) -> Result<ValidatorResponse, ActorError> {
        match msg {
            ValidatorCommand::LocalValidation((validation_req, key_identifier)) => {
                // Validar y dar respuesta.
                Ok(ValidatorResponse::None)
            },
            ValidatorCommand::NetworkValidation((validation_req, key_identifier)) => {
                // Lanzar evento donde lanzar los retrys
                Ok(ValidatorResponse::None)
            },
            ValidatorCommand::NetworkResponse => {
                // Recibir respuesta de la network, si no es un error darle la respuesta al padre y si es error no hacer nada.
                Ok(ValidatorResponse::None)
            }
        }
    }

    async fn on_event(
        &mut self,
        event: ValidatorEvent,
        ctx: &mut ActorContext<Validator>,
    ) {
        match event {
            ValidatorEvent::AllTryHaveBeenMade => {
                // Si llega este evento y is_finished omitir (la respuesta llegó después de terminar los eventos)
                // Si no está is_finished mandar un mensaje al padre con que No se pudo completar la validación para este actor, se supone que se da por validado.
            },
            ValidatorEvent::ReTry => {
                // Lanzar retrys (bloqueante)
            }
        }
    }
}

#[async_trait]
impl Retry for Validator {
    type Child = RetryValidator;

    async fn child(
        &self,
        ctx: &mut ActorContext<Validator>,
    ) -> Result<ActorRef<RetryValidator>, ActorError> {
        ctx.create_child("network", RetryValidator {})
            .await
    }

        /// Retry message.
        async fn apply_retries(
            &self,
            ctx: &mut ActorContext<Self>,
            path: ActorPath,
            retry_strategy: &mut Strategy,
            message: <<Self as Retry>::Child as Actor>::Message,
        ) -> Result<<<Self as Retry>::Child as Actor>::Response, ActorError> {
            if let Ok(child) = self.child(ctx).await {
                let mut retries = 0;
                while retries < retry_strategy.max_retries() && !self.is_finished() {
                    debug!(
                        "Retry {}/{}.",
                        retries + 1,
                        retry_strategy.max_retries()
                    );
                    if let Err(e) = child.tell(message.clone()).await {
                        error!("");
                        // Manejar error del tell.
                    } else {
                        if let Some(duration) = retry_strategy.next_backoff() {
                            debug!("Backoff for {:?}", &duration);
                            tokio::time::sleep(duration).await;
                        }
                        retries += 1;
                    }
                }
                error!("Max retries with actor {} reached.", path);
                // emitir evento de que todos los intentos fueron realizados
                Err(ActorError::Functional(format!(
                    "Max retries with actor {} reached.",
                    path
                )))
            } else {
                error!("Retries with actor {} failed. Unknown actor.", path);
                Err(ActorError::Functional(format!(
                    "Retries with actor {} failed. Unknown actor.",
                    path
                )))
            }
        }
}


#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct RetryValidator {}

#[derive(Debug, Clone)]
pub struct RetryValidatorCommand {}

impl Message for RetryValidatorCommand {}


#[async_trait]
impl Actor for RetryValidator {
    type Event = ValidatorEvent;
    type Message = RetryValidatorCommand;
    type Response = ValidatorResponse;
}

#[async_trait]
impl Handler<RetryValidator> for RetryValidator {
    async fn handle_message(
        &mut self,
        msg: RetryValidatorCommand,
        ctx: &mut ActorContext<RetryValidator>,
    ) -> Result<ValidatorResponse, ActorError> {

        // Enviar mensaje a la network
        Ok(ValidatorResponse::None)
    }
}
