// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Validation module.
//!

pub mod proof;
pub mod request;
pub mod response;
pub mod validator;

use crate::{
    governance::Governance, model::{
        event::Event as KoreEvent, request::EventRequest, signature::{Signature, Signed},
        HashId, Namespace,
    }, subject::SubjectState, Error, DIGEST_DERIVATOR
};
use actor::{Actor, ActorContext, Message, Event, Response, Handler, Error as ActorError};

use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::{
    derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
};
use request::ValidationRequest;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tracing::{debug, error};

use std::collections::HashSet;

pub struct Validator {
    subject: SubjectState,
    governance: Governance,
    event: Signed<KoreEvent>,
}

/// A struct for passing validation information.
#[derive(Clone)]
pub struct ValidationInfo {
    pub subject: SubjectState,
    pub event: Signed<KoreEvent>,
    pub gov_version: u64,
}

/* 
impl Message for ValidationCommand {}

impl Event for ValidationEvent {}

impl Response for ValidationResponse {}

impl Actor for Validator {
    type Message = ValidationCommand;
    type Event = ValidationEvent;
    type Response = ValidationResponse;
}

#[async_trait]
impl Handler<Validator> for Validator {

    async fn handle_message(
        &mut self,
        message: ValidationCommand,
        ctx: &mut ActorContext<Validator>,
    ) -> Result<ValidationResponse, ActorError> {
        match message {
            ValidationCommand::Create(info) => {
                Ok(ValidationResponse::None)
            }
            ValidationCommand::Validate(event) => {
                // Validate the event.
                // If the event is valid, return the validation signature.
                // If the event is invalid, return an error.
                Ok(ValidationResponse::None)
            }
        }
    }

}

/// A struct for passing validation information.
#[derive(Clone)]
pub struct ValidationInfo {
    pub subject: SubjectState,
    pub event: Signed<KoreEvent>,
    pub gov_version: u64,
}
#[derive(Clone)]
pub enum ValidationCommand {
    Create(ValidationInfo),
    Validate(ValidationRequest),
    Response(ValidationResponse),
}

/// A struct representing a validation response.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
)]
pub enum ValidationResponse {
    Signature{
        validation_signature: Signature,
        gov_version_validation: u64,
    },
    None,
}
*/