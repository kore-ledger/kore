// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Validation module.
//!

pub mod proof;
pub mod request;
pub mod response;
pub mod validator;

use crate::{
    db::Storable, governance::Governance, model::{
        event::Event as KoreEvent,
        request::EventRequest,
        signature::{Signature, Signed},
        HashId, Namespace,
    }, subject::SubjectState, Error, DIGEST_DERIVATOR
};
use actor::{
    Actor, ActorContext, Error as ActorError, Event, Handler, Message, Response,
};

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::{
    derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
};
use request::ValidationReq;
use response::ValidationRes;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};

use std::collections::HashSet;

/// A struct for passing validation information.
#[derive(Clone, Debug)]
pub struct ValidationInfo {
    pub subject: SubjectState,
    pub event: Signed<KoreEvent>,
    pub gov_version: u64,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Validation {
    // Quorum
    // A quien preguntar
    // Respuestas
}

#[derive(Debug, Clone)]
pub enum ValidationCommand {
    Create(ValidationInfo),
    Response((ValidationResponse, KeyIdentifier)),
}

impl Message for ValidationCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationEvent {}

impl Event for ValidationEvent {}

#[derive(Debug, Clone)]
pub enum ValidationResponse {
    Response(ValidationRes),
    None,
}

impl Response for ValidationResponse {}

#[async_trait]
impl Actor for Validation {
    type Event = ValidationEvent;
    type Message = ValidationCommand;
    type Response = ValidationResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Starting validation actor with init store.");
        self.init_store("validation", true, ctx).await
    }

    async fn post_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Stopping validation actor with stop store.");
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<Validation> for Validation {
    async fn handle_message(
        &mut self,
        msg: ValidationCommand,
        ctx: &mut ActorContext<Validation>,
    ) -> Result<ValidationResponse, ActorError> {
        match msg {
            ValidationCommand::Create(validation_info) => {
                // Validation info a validation req,
                // Mirar quien tiene que evaluar
                // Si va local (realizar ask a node) child Validator Local
                // Si va fuera child Validator Network
                Ok(ValidationResponse::None)
            },
            ValidationCommand::Response(response) => {
                // Mirar qué hijo ha respondido
                // Eliminarlo de la lista de pendientes
                // hay que mirar que las validaciones sean todas iguales ¿?
                // Comprar quorum, si quorum >= respuesta al padre (request)
                // Los Hijos Retry podran responder None si el validador no responde en X tiempo, manejar eso.

                Ok(ValidationResponse::None)
            }
        }
    }
}

#[async_trait]
impl PersistentActor for Validation {
    fn apply(&mut self, event: &ValidationEvent) {
        
    }
}

impl Storable for Validation {}