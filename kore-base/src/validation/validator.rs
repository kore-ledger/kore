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
    Actor, ActorContext, ActorRef, Error as ActorError, Event, Handler, Message, Response, Retry
};

use tracing::{debug, error};

/// A struct representing a validator actor.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Validator {}

impl Validator {
    async fn validation_event() {

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
pub enum ValidatorEvent {}

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
                // Lanzar retrys
                Ok(ValidatorResponse::None)
            },
            ValidatorCommand::NetworkResponse => {
                // Recibir respuesta de la network, si no es un error darle la respuesta al padre y si es error no hacer nada.
                Ok(ValidatorResponse::None)
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
