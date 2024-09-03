// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance module.
//!

pub mod init;
pub mod json_schema;
pub mod model;
mod schema;

use crate::{
    db::{Database, Storable},
    model::{request, wrapper::ValueWrapper, Namespace},
    subject::SubjectState,
    Error,
};

use model::{Contract, Roles};
pub use schema::schema;

pub use model::{
    GovernanceModel, Member, Policy, Quorum, RequestStage, Role, Schema, Who,
};

use identity::{
    identifier::{DigestIdentifier, KeyIdentifier},
    keys::KeyPair,
};

use actor::{
    Actor, ActorContext, Error as ActorError, Event, Handler, Message, Response,
};

use store::{database::DbManager, store::PersistentActor};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use std::{collections::{HashMap, HashSet}, str::FromStr, sync::atomic::AtomicU64};

/// Governance struct.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Gets initial state of the subject from governance
    ///
    pub fn get_initial_state(
        &self,
        schema_id: &str,
        owner: &str,
    ) -> Result<ValueWrapper, Error> {
        if self.governance_id.digest.is_empty() {
            debug!("Meta-governance initial state.");
            return Ok(init::init_state(owner));
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
    pub fn get_schema(&self, schema_id: &str) -> Result<Schema, Error> {
        for schema in &self.model.schemas {
            debug!("Schema found: {}", schema_id);
            if &schema.id == schema_id {
                return Ok(schema.clone());
            }
        }
        error!("Schema not found: {}", schema_id);
        Err(Error::Governance("Schema not found.".to_owned()))
    }

    fn members_to_key_identifier(&self) -> HashSet<KeyIdentifier> {
        HashSet::from_iter(
            self.model
                .members
                .iter()
                .filter_map(|e| KeyIdentifier::from_str(&e.id).ok()),
        )
    }

    fn id_by_name(&self, name: &str) -> Option<String> {
        let member = self.model.members.iter().find(|e| &e.name == name);
        if let Some(member) = member {
            Some(member.id.clone())
        } else {
            None
        }
    }

    /// Gets the signers for the request stage.
    pub fn get_signers(
        &self,
        role: Roles,
        schema: &str,
        namespace: Namespace,
    ) -> HashSet<KeyIdentifier> {
        let mut signers = HashSet::new();
        for rol in &self.model.roles {
            // Check if the stage is for the role.
            if role == rol.role {
                // Check namespace
                let namespace_role = Namespace::from(rol.namespace.to_string());
                if !namespace_role.is_ancestor_of(&namespace)
                    && namespace_role != namespace
                    && !namespace_role.is_empty()
                {
                    continue;
                }

                match rol.schema.clone() {
                    // Check rol for schema
                    model::SchemaEnum::ALL => {
                        // We do nothing, the role applies to all schemes.
                    }
                    model::SchemaEnum::ID { ID } => {
                        if schema != &ID {
                            continue;
                        }
                    }
                    model::SchemaEnum::NOT_GOVERNANCE => {
                        if schema == "governance" {
                            continue;
                        }
                    }
                }
                match rol.who.clone() {
                    Who::MEMBERS => {
                        signers = self.members_to_key_identifier();
                        break;
                    }
                    Who::ID { ID } => {
                        if let Ok(id) = KeyIdentifier::from_str(&ID) {
                            let _ = signers.insert(id);
                        }
                    }

                    Who::NAME { NAME } => {
                        let id_string = self.id_by_name(&NAME);
                        if let Some(id) = id_string {
                            if let Ok(id) = KeyIdentifier::from_str(&id) {
                                let _ = signers.insert(id);
                            }
                        }
                    }
                    Who::NOT_MEMBERS => {
                        // If it is not a member, we will not have a public key.
                    }
                }
            }
        }

        signers
    }

    fn get_quorum(
        &self,
        stage: RequestStage,
        schema: &str,
    ) -> Result<Quorum, Error> {
        let policies = self.model.policies.iter().find(|e| e.id == schema);
        if let Some(policies) = policies {
            match stage {
                RequestStage::Evaluate => {
                    debug!("");
                    Ok(policies.evaluate.quorum.clone())
                }
                RequestStage::Approve => {
                    debug!("");
                    Ok(policies.approve.quorum.clone())
                }
                RequestStage::Validate => {
                    debug!("");
                    Ok(policies.validate.quorum.clone())
                }
                _ => {
                    error!("");
                    Err(Error::InvalidQuorum(
                        "No Validate quorum found for this scheme".to_owned(),
                    ))
                }
            }
        } else {
            error!("");
            Err(Error::InvalidQuorum(
                "No Evaluate quorum found for this scheme".to_owned(),
            ))
        }
    }

    pub fn get_quorum_and_signers(
        &self,
        stage: RequestStage,
        schema: &str,
        namespace: Namespace,
    ) -> Result<(HashSet<KeyIdentifier>, Quorum), Error> {
        let signers = self.get_signers(stage.to_role(), schema, namespace);
        let quorum = self.get_quorum(stage, schema);
        match quorum {
            Ok(quorum) => Ok((signers, quorum)),
            Err(e) => Err(e),
        }
    }

    pub fn subjects_schemas_rol_namespace(&self) -> (HashMap<(String, Roles), Vec<String>>, HashMap<(String, String), Vec<String>>){
        let subject = self.subject_id.to_string();
        let mut our_roles: HashMap<(String, Roles), Vec<String>> = HashMap::new();
        let mut creators: HashMap<(String, String), Vec<String>> = HashMap::new();
        let all_schemas: Vec<String> = self.model.schemas.iter().map(|x| x.id.clone()).collect();
        let all_members: Vec<String> = self.model.members.iter().map(|x| x.id.clone()).collect();

        for rol in self.model.roles.clone() {
            let mut schema: String = String::default();
            let mut is_me: bool = false;
            let mut is_all: bool = false;
            let mut member: String = String::default();

            match rol.who {
                Who::ID { ID } => {
                    if subject == ID {
                        is_me = true;
                    } else {
                        member = ID;
                    }
                },
                Who::NAME { NAME } => {
                    let id_string = self.id_by_name(&NAME);
                        if let Some(id) = id_string {
                            if subject == id {
                                is_me = true;
                            } else {
                                member = id;
                            }
                        } else {
                            continue;
                        }
                },
                Who::NOT_MEMBERS => continue,
                Who::MEMBERS => is_all = true
            };

            match rol.schema {
                model::SchemaEnum::ID { ID } => {
                    if ID != "governance" {
                        schema = ID;
                    } else {
                        continue;
                    }
                },
                _ => schema = "NOT_GOVERNANCE".to_owned(),
            }

            match rol.role {
                Roles::APPROVER | Roles::EVALUATOR | Roles::VALIDATOR => {
                    if is_me || is_all {
                        if schema == "NOT_GOVERNANCE" {
                            for schema in all_schemas.clone() {
                                if let Some(state) = our_roles.get(&(schema.clone(), rol.role.clone())) {
                                    let mut state = state.clone();
                                    state.push(rol.namespace.clone());
                                    our_roles.insert((schema, rol.role.clone()),state);
                                } else {
                                    our_roles.insert((schema, rol.role.clone()), vec![rol.namespace.clone()]);
                                }
                            }
                        } else {
                            if let Some(state) = our_roles.get(&(schema.clone(), rol.role.clone())) {
                                let mut state = state.clone();
                                state.push(rol.namespace);
                                our_roles.insert((schema, rol.role),state);
                            } else {
                                our_roles.insert((schema, rol.role), vec![rol.namespace]);
                            }
                        }
                    }
                },
                Roles::CREATOR => {
                    if !is_me {
                        if schema == "NOT_GOVERNANCE" {
                            for schema in all_schemas.clone() {
                                if let Some(state) = creators.get(&(schema.clone(), member.clone())) {
                                    let mut state = state.clone();
                                    state.push(rol.namespace.clone());
                                    creators.insert((schema, member.clone()),state);
                                } else {
                                    creators.insert((schema, member.clone()), vec![rol.namespace.clone()]);
                                }
                            }
                        } else {
                            if let Some(state) = creators.get(&(schema.clone(), member.clone())) {
                                let mut state = state.clone();
                                state.push(rol.namespace);
                                creators.insert((schema, member),state.clone());
                            } else {
                                creators.insert((schema, member), vec![rol.namespace]);
                            }  
                        }
                    } else if is_all {
                        for member in all_members.clone() {
                            if schema == "NOT_GOVERNANCE" {
                                for schema in all_schemas.clone() {
                                    if let Some(state) = creators.get(&(schema.clone(), member.clone())) {
                                        let mut state = state.clone();
                                        state.push(rol.namespace.clone());
                                        creators.insert((schema, member.clone()),state);
                                    } else {
                                        creators.insert((schema, member.clone()), vec![rol.namespace.clone()]);
                                    }
                                }
                            } else {
                                if let Some(state) = creators.get(&(schema.clone(), member.clone())) {
                                    let mut state = state.clone();
                                    state.push(rol.namespace.clone());
                                    creators.insert((schema.clone(), member),state.clone());
                                } else {
                                    creators.insert((schema.clone(), member), vec![rol.namespace.clone()]);
                                }  
                            }
                        }
                    }
                },
                _ => {}
            };
            
        }
        (our_roles, creators)
    }

    pub fn get_shcemas(&self) -> Vec<Schema> {
        self.model.schemas.clone()
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
                }
            }
        }
        false
    }

    /// Governance version.
    pub fn get_version(&self) -> u64 {
        self.model.version
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
        let model: GovernanceModel =
            serde_json::from_value(subject.properties.0).map_err(|_| {
                Error::Governance("Governance model not found.".to_owned())
            })?;
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
