// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Node module
//!

use crate::{
    db::{Database, Storable},
    helpers::encrypted_pass::EncryptedPass,
    model::{request::EventRequest, signature::Signed},
    Api, Config, Error,
};

use identity::{
    identifier::{DigestIdentifier, KeyIdentifier},
    keys::{KeyMaterial, KeyPair},
};

use actor::{
    Actor, ActorContext, ActorSystem, Error as ActorError, Event, Handler,
    Message, Response, SystemRef,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};

/// Node struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    /// Owner of the node.
    #[serde(skip)]
    owner: KeyPair,
    /// The node's owned subjects.
    owned_subjects: Vec<String>,
    /// The node's known subjects.
    known_subjects: Vec<String>,
    /// The node's owned governances.
    owned_governances: Vec<String>,
    /// The node's known governances.
    known_governances: Vec<String>,
}

impl Node {
    /// Creates a new node.
    pub fn new(id: &KeyPair) -> Result<Self, Error> {
        Ok(Self {
            owner: id.clone(),
            owned_subjects: Vec::new(),
            known_subjects: Vec::new(),
            owned_governances: Vec::new(),
            known_governances: Vec::new(),
        })
    }

    /// Gets the node's owner identifier.
    ///
    /// # Returns
    ///
    /// A `KeyIdentifier` with the node's owner identifier.
    ///
    pub fn owner(&self) -> KeyIdentifier {
        self.owner.key_identifier()
    }

    /// Adds a subject to the node's known subjects.
    pub fn add_known_subject(&mut self, subject_id: String) {
        self.known_subjects.push(subject_id);
    }

    /// Adds a subject to the node's owned subjects.
    pub fn add_owned_subject(&mut self, subject_id: String) {
        self.owned_subjects.push(subject_id);
    }

    /// Adds a governance to the node's known governances.
    pub fn add_known_governance(&mut self, governance_id: String) {
        self.known_governances.push(governance_id);
    }

    /// Adds a governance to the node's owned governances.
    pub fn add_owned_governance(&mut self, governance_id: String) {
        self.owned_governances.push(governance_id);
    }

    /// Gets the node's owned subjects.
    pub fn get_owned_subjects(&self) -> &Vec<String> {
        &self.owned_subjects
    }

    /// Gets the node's known subjects.
    pub fn get_known_subjects(&self) -> &Vec<String> {
        &self.known_subjects
    }

    /// Gets the node's owned governances.
    pub fn get_owned_governances(&self) -> &Vec<String> {
        &self.owned_governances
    }

    /// Gets the node's known governances.
    pub fn get_known_governances(&self) -> &Vec<String> {
        &self.known_governances
    }
}

/// Node message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeMessasge {
    RequestEvent(Signed<EventRequest>),
    GetOwnerIdentifier,
}

impl Message for NodeMessasge {}

/// Node response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeResponse {
    /// Event request.
    RequestIdentifier(Result<DigestIdentifier, Error>),
    /// Owner identifier.
    OwnerIdentifier(KeyIdentifier),
    None,
}

impl Response for NodeResponse {}

/// Node event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeEvent {
    OwnedSubject(String),
    KnownSubject(String),
    OwnedGovernance(String),
    KnownGovernance(String),
}

impl Event for NodeEvent {}

#[async_trait]
impl Actor for Node {
    type Event = NodeEvent;
    type Message = NodeMessasge;
    type Response = NodeResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut actor::ActorContext<Self>,
    ) -> Result<(), ActorError> {
        // Start store
        debug!("Creating Node store");
        self.init_store("node", false, ctx).await?;

        Ok(())
    }

    async fn post_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Stopping Node store");
        self.stop_store(ctx).await?;
        Ok(())
    }
}

#[async_trait]
impl PersistentActor for Node {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) {
        match event {
            NodeEvent::OwnedSubject(subject_id) => {
                self.add_owned_subject(subject_id.clone());
            }
            NodeEvent::KnownSubject(subject_id) => {
                self.add_known_subject(subject_id.clone());
            }
            NodeEvent::OwnedGovernance(governance_id) => {
                self.add_owned_governance(governance_id.clone());
            }
            NodeEvent::KnownGovernance(governance_id) => {
                self.add_known_governance(governance_id.clone());
            }
        }
    }

    /// Override the update method.
    fn update(&mut self, state: Self) {
        self.owned_subjects = state.owned_subjects;
        self.known_subjects = state.known_subjects;
        self.owned_governances = state.owned_governances;
        self.known_governances = state.known_governances;
    }
}

#[async_trait]
impl Handler<Node> for Node {
    async fn handle_message(
        &mut self,
        msg: NodeMessasge,
        _ctx: &mut actor::ActorContext<Node>,
    ) -> Result<NodeResponse, ActorError> {
        match msg {
            NodeMessasge::RequestEvent(event) => Ok(NodeResponse::None),
            NodeMessasge::GetOwnerIdentifier => {
                Ok(NodeResponse::OwnerIdentifier(self.owner()))
            }
        }
    }
}

#[async_trait]
impl Storable for Node {}

#[cfg(test)]
pub mod tests {

    use super::*;
}
