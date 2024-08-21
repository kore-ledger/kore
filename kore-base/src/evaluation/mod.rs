// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Evaluation module.
//! This module contains the evaluation logic for the Kore protocol.
//!

pub mod request;
pub mod response;
pub mod evaluator;
mod runner;
mod compiler;

use crate::{
    db::Storable,
    governance::{
        Governance, Quorum, RequestStage,
    },
    model::{
        event::Event as KoreEvent,
        namespace,
        request::EventRequest,
        signature::{self, Signature, Signed},
        HashId, Namespace, SignTypes,
    },
    node::{Node, NodeMessage, NodeResponse},
    subject::{Subject, SubjectCommand, SubjectResponse, SubjectState},
    Error, DIGEST_DERIVATOR,
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::{
    derive::digest::DigestDerivator, key_identifier, DigestIdentifier,
    KeyIdentifier,
};
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};
use wasmtime::Engine;


use std::{collections::HashSet, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Evaluation {}

impl Evaluation {}

#[derive(Debug, Clone)]
pub enum EvaluationCommand {}

impl Message for EvaluationCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationEvent {}

impl Event for EvaluationEvent {}

#[derive(Debug, Clone)]
pub enum EvaluationResponse {
    Error(Error),
    None
}

impl Response for EvaluationResponse {}

#[async_trait]
impl Actor for Evaluation {
    type Event = EvaluationEvent;
    type Message = EvaluationCommand;
    type Response = EvaluationResponse;

}

// TODO: revizar todos los errores, algunos pueden ser ActorError.
#[async_trait]
impl Handler<Evaluation> for Evaluation {
    async fn handle_message(
        &mut self,
        msg: EvaluationCommand,
        ctx: &mut ActorContext<Evaluation>,
    ) -> Result<EvaluationResponse, ActorError> {

                Ok(EvaluationResponse::None)
        }
    async fn on_event(
        &mut self,
        event: EvaluationEvent,
        ctx: &mut ActorContext<Evaluation>,
    ) {}
}