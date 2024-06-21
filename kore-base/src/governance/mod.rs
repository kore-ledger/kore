// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance module.
//!

mod init;
mod model;
mod schema;

use crate::{model::{request, wrapper::ValueWrapper, Namespace}, subject::SubjectState, Error};

pub use schema::schema;

use model::{GovernanceModel, Member, Policy, Quorum, RequestStage, Role, Schema, Who};

use identity::{
    identifier::{DigestIdentifier, KeyIdentifier},
    keys::KeyPair,
};

use actor::{Actor, Handler, Message, Response, Event, ActorContext};

use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tracing::{debug, error};

use std::{
    collections::HashSet,
    sync::atomic::AtomicU64,
};

/// Governance struct.
///
#[derive(Serialize, Deserialize)]
pub struct Governance {
    /// The identifier of the governance.
    subject_id: DigestIdentifier,
    /// The identifier of the governance that drives this subject.
    governance_id: DigestIdentifier,
    /// The namespace of the subject.
    namespace: Namespace,
    /// The name of the subject.
    name: String,
    /// Indicates whether the governace is active or not.
    active: bool,
    /// The current sequence number of the events.
    sn: u64,
    /// The governance model.
    model: GovernanceModel,
}

impl Governance {

    /// Creates a new `Governance`.
    pub fn new() -> Result<Self, Error> {
        let model: GovernanceModel = serde_json::from_value(init::init_state().0)
            .map_err(|_| Error::Governance("Model not found.".to_owned()))?;

        Ok(Governance {
            subject_id: DigestIdentifier::default(),
            governance_id: DigestIdentifier::default(),
            namespace: Namespace::default(),
            name: "".to_owned(),
            active: true,
            sn: 0,
            model,
        })
    }

    /// Gets initial state of the subject from governance
    ///
    pub fn get_initial_state(
        &self,
        schema_id: &str,
    ) -> Result<ValueWrapper, Error> {
        if self.governance_id.digest.is_empty() {
            debug!("Meta-governance initial state.");
            return Ok(init::init_state());
        }
        for schema in &self.model.schemas {
            if &schema.id == schema_id {
                debug!("Schema found: {}", schema_id);
                return Ok(ValueWrapper(schema.initial_value.clone()));
            }
        }
        error!("Schema not found: {}", schema_id);
        Err(Error::Governance("Schema not found.".to_owned()))
    }

    /// Get the schema by id.
    ///
    pub fn get_schema(&self, schema_id: &str) -> Result<&Schema, Error> {
        for schema in &self.model.schemas {
            debug!("Schema found: {}", schema_id);
            if &schema.id == schema_id {
                return Ok(schema);
            }
        }
        error!("Schema not found: {}", schema_id);
        Err(Error::Governance("Schema not found.".to_owned()))
    }

    /// Gets the signers for the request stage.
    pub fn get_signers(&self, stage: RequestStage) -> Vec<DigestIdentifier> {
        let mut signers = Vec::new();
        // Create hashset of members.
        let members: HashSet<Member> = self.model.members.iter().cloned().collect(); 
        for rol in &self.model.roles {
            // Check if the stage is for the role.
            if stage.to_role() == &rol.role {

            }
        }
        /*for policy in &self.model.policies {
            if policy.stage == stage {
                for role in &policy.roles {
                    for member in &role.members {
                        signers.push(member.id.clone());
                    }
                }
            }
        }*/
        signers
    }

    /// Check if the request is allowed.
    pub fn is_allowed(
        &self,
        id: KeyIdentifier,
        name: &str,
        stage: RequestStage,
    ) -> bool {
        for rol in &self.model.roles {
            if rol.role == stage.to_role() {
                match &rol.who {
                    Who::ID { ID } => return &id.to_string() == ID,
                    Who::NAME { NAME } => return name == NAME,
                    Who::MEMBERS => return self.is_member(&id),
                    Who::NOT_MEMBERS => return !self.is_member(&id),
                    Who::ALL => return true,
                }
            }
        }
        false
    }

    /// Check if the key is a member.
    fn is_member(&self, id: &KeyIdentifier) -> bool {
        for member in &self.model.members {
            if &member.id == &id.to_string() {
                return true;
            }
        }
        false
    }
}

impl TryFrom<SubjectState> for Governance {
    type Error = Error;

    fn try_from(subject: SubjectState) -> Result<Self, Self::Error> {
        let model: GovernanceModel = serde_json::from_value(subject.properties.0)
            .map_err(|_| Error::Governance("Model not found.".to_owned()))?;
        Ok(Governance {
            subject_id: subject.subject_id,
            governance_id: subject.governance_id,
            namespace: subject.namespace,
            name: subject.name,
            active: subject.active,
            sn: subject.sn,
            model,
        })
    }
}

/// Governance command.
#[derive(Debug, Clone)]
pub enum GovernanceCommand {
    GetSchema { schema_id: String },
    GetInitialState { schema_id: String },
    GetSigners { stage: RequestStage, schema_id: String, namespace: Namespace },
    IsAllowed { id: KeyIdentifier, stage: RequestStage, name: String },
}

impl Message for GovernanceCommand {}

/// Governance response.
#[derive(Debug, Clone)]
pub enum GovernanceResponse {
    Schema(Schema),
    InitialState(ValueWrapper),
    Signers(Vec<DigestIdentifier>, Quorum),
    Allow(bool),
    None,
}

impl Response for GovernanceResponse {}

/// Governance event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GovernanceEvent {
    Create,
    Update,
    Delete { governance_id: DigestIdentifier },
}

impl Event for GovernanceEvent {}

/// Actor implementation for `Governance`.
#[async_trait]
impl Actor for Governance {
    type Event = GovernanceEvent;
    type Message = GovernanceCommand;
    type Response = GovernanceResponse;
}

/// Handler implementation for `Governance`.
#[async_trait]
impl Handler<Governance> for Governance {
    async fn handle_message(
        &mut self,
        msg: GovernanceCommand,
        ctx: &mut ActorContext<Governance>,
    ) -> GovernanceResponse {
        match msg {
            GovernanceCommand::IsAllowed { id, stage, name } => 
                GovernanceResponse::Allow(self.is_allowed(id, &name, stage)),
        
            _ => GovernanceResponse::None,
        } 
    }
}