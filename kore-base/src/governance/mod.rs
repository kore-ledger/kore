// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance module.
//!

// TODO Cuando haya un cambio en la governanza, en un schema para el cual soy evaluador, tengo que realizar la compilación de JSONSchema y compilación del contrato,
// solo en el caso de que haya ocurrido algún cambio.
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

use json_schema::JsonSchema;
use model::{Contract, Roles};
pub use schema::schema;

pub use model::{Member, Policy, Quorum, RequestStage, Role, Schema, Who};

use identity::{
    identifier::{DigestIdentifier, KeyIdentifier},
    keys::KeyPair,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::atomic::AtomicU64,
};

/// Governance struct.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Governance {
    /// The version of the governance
    #[serde(default)]
    pub version: u64,
    /// The set of subjects identifiers directed by this governance.
    #[serde(default)]
    pub subjects_id: HashSet<DigestIdentifier>,
    /// The set of members.
    pub members: Vec<Member>,
    /// The set of roles.
    pub roles: Vec<Role>,
    /// The set of schemas.
    pub schemas: Vec<Schema>,
    /// The set of policies.
    pub policies: Vec<Policy>,
}

impl Governance {
    pub fn get_init_state(
        &self,
        schema_id: &str,
    ) -> Result<ValueWrapper, Error> {
        for schema in &self.schemas {
            if schema.id == schema_id {
                debug!("Schema found: {}", schema_id);
                return Ok(ValueWrapper(schema.initial_value.clone()));
            }
        }

        Err(Error::Governance("Schema not found.".to_owned()))
    }

    /// Get the schema by id.
    ///
    pub fn get_schema(&self, schema_id: &str) -> Result<Schema, Error> {
        for schema in &self.schemas {
            debug!("Schema found: {}", schema_id);
            if schema.id == schema_id {
                return Ok(schema.clone());
            }
        }
        error!("Schema not found: {}", schema_id);
        Err(Error::Governance("Schema not found.".to_owned()))
    }

    pub fn members_to_key_identifier(&self) -> HashSet<KeyIdentifier> {
        HashSet::from_iter(
            self.members
                .iter()
                .filter_map(|e| KeyIdentifier::from_str(&e.id).ok()),
        )
    }

    fn id_by_name(&self, name: &str) -> Option<String> {
        let member = self.members.iter().find(|e| e.name == name);
        member.map(|member| member.id.clone())
    }

    /// Gets the signers for the request stage.
    pub fn get_signers(
        &self,
        role: Roles,
        schema: &str,
        namespace: Namespace,
    ) -> HashSet<KeyIdentifier> {
        let mut signers = HashSet::new();
        for rol in &self.roles {
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
                        if schema != ID {
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

    fn get_quorum(&self, role: Roles, schema: &str) -> Result<Quorum, Error> {
        let policies = self.policies.iter().find(|e| e.id == schema);
        if let Some(policies) = policies {
            match role {
                Roles::APPROVER => Ok(policies.approve.quorum.clone()),
                Roles::EVALUATOR => Ok(policies.evaluate.quorum.clone()),
                Roles::VALIDATOR => Ok(policies.validate.quorum.clone()),
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
        role: Roles,
        schema: &str,
        namespace: Namespace,
    ) -> Result<(HashSet<KeyIdentifier>, Quorum), Error> {
        let signers = self.get_signers(role.clone(), schema, namespace);
        let quorum = self.get_quorum(role, schema);
        match quorum {
            Ok(quorum) => Ok((signers, quorum)),
            Err(e) => Err(e),
        }
    }

    pub fn schemas(&self, role: Roles, our_id: &str) -> Vec<Schema> {
        let mut schemas_id: Vec<String> = vec![];
        let mut all_schemas = false;

        for rol in self.roles.clone() {
            match rol.who {
                Who::ID { ID } => {
                    if our_id != ID {
                        continue;
                    }
                }
                Who::NAME { NAME } => {
                    let id_string = self.id_by_name(&NAME);
                    if let Some(id) = id_string {
                        if our_id != id {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                Who::NOT_MEMBERS => continue,
                Who::MEMBERS => {}
            };

            if rol.role == role {
                match rol.schema {
                    model::SchemaEnum::ID { ID } => {
                        if ID != "governance" {
                            schemas_id.push(ID);
                        } else {
                            continue;
                        }
                    }
                    _ => {
                        all_schemas = true;
                        break;
                    }
                }
            }
        }

        if all_schemas {
            return self.schemas.clone();
        } else {
            let mut schemas: Vec<Schema> = vec![];
            for id in schemas_id {
                for schema in self.schemas.clone() {
                    if id == schema.id {
                        schemas.push(schema);
                    }
                }
            }

            return schemas;
        }
    }

    pub fn subjects_schemas_rol_namespace(
        &self,
        our_id: &str,
    ) -> (
        HashMap<(String, Roles), Vec<String>>,
        HashMap<(String, String), Vec<String>>,
    ) {
        let mut our_roles: HashMap<(String, Roles), Vec<String>> =
            HashMap::new();
        let mut creators: HashMap<(String, String), Vec<String>> =
            HashMap::new();
        let all_schemas: Vec<String> =
            self.schemas.iter().map(|x| x.id.clone()).collect();
        let all_members: Vec<String> =
            self.members.iter().map(|x| x.id.clone()).collect();

        for rol in self.roles.clone() {
            let schema: String;
            let mut is_me = false;
            let mut is_all = false;
            let mut member = String::default();

            match rol.who {
                Who::ID { ID } => {
                    if our_id == ID {
                        is_me = true;
                    } else {
                        member = ID;
                    }
                }
                Who::NAME { NAME } => {
                    let id_string = self.id_by_name(&NAME);
                    if let Some(id) = id_string {
                        if our_id == id {
                            is_me = true;
                        } else {
                            member = id;
                        }
                    } else {
                        continue;
                    }
                }
                Who::NOT_MEMBERS => continue,
                Who::MEMBERS => is_all = true,
            };

            match rol.schema {
                model::SchemaEnum::ID { ID } => {
                    if ID != "governance" {
                        schema = ID;
                    } else {
                        continue;
                    }
                }
                _ => schema = "NOT_GOVERNANCE".to_string(),
            }

            match rol.role {
                Roles::APPROVER | Roles::EVALUATOR | Roles::VALIDATOR => {
                    if is_me || is_all {
                        if schema == "NOT_GOVERNANCE" {
                            for schema in all_schemas.clone() {
                                if let Some(state) = our_roles
                                    .get(&(schema.clone(), rol.role.clone()))
                                {
                                    let mut state = state.clone();
                                    state.push(rol.namespace.clone());
                                    our_roles.insert(
                                        (schema, rol.role.clone()),
                                        state,
                                    );
                                } else {
                                    our_roles.insert(
                                        (schema, rol.role.clone()),
                                        vec![rol.namespace.clone()],
                                    );
                                }
                            }
                        } else if let Some(state) =
                            our_roles.get(&(schema.clone(), rol.role.clone()))
                        {
                            let mut state = state.clone();
                            state.push(rol.namespace);
                            our_roles.insert((schema, rol.role), state);
                        } else {
                            our_roles.insert(
                                (schema, rol.role),
                                vec![rol.namespace],
                            );
                        }
                    }
                }
                Roles::CREATOR { quantity } => {
                    if !is_me {
                        if schema == "NOT_GOVERNANCE" {
                            for schema in all_schemas.clone() {
                                if let Some(state) = creators
                                    .get(&(schema.clone(), member.clone()))
                                {
                                    let mut state = state.clone();
                                    state.push(rol.namespace.clone());
                                    creators.insert(
                                        (schema, member.clone()),
                                        state,
                                    );
                                } else {
                                    creators.insert(
                                        (schema, member.clone()),
                                        vec![rol.namespace.clone()],
                                    );
                                }
                            }
                        } else if let Some(state) =
                            creators.get(&(schema.clone(), member.clone()))
                        {
                            let mut state = state.clone();
                            state.push(rol.namespace);
                            creators.insert((schema, member), state.clone());
                        } else {
                            creators
                                .insert((schema, member), vec![rol.namespace]);
                        }
                    } else if is_all {
                        for member in all_members.clone() {
                            if schema == "NOT_GOVERNANCE" {
                                for schema in all_schemas.clone() {
                                    if let Some(state) = creators
                                        .get(&(schema.clone(), member.clone()))
                                    {
                                        let mut state = state.clone();
                                        state.push(rol.namespace.clone());
                                        creators.insert(
                                            (schema, member.clone()),
                                            state,
                                        );
                                    } else {
                                        creators.insert(
                                            (schema, member.clone()),
                                            vec![rol.namespace.clone()],
                                        );
                                    }
                                }
                            } else if let Some(state) =
                                creators.get(&(schema.clone(), member.clone()))
                            {
                                let mut state = state.clone();
                                state.push(rol.namespace.clone());
                                creators.insert(
                                    (schema.clone(), member),
                                    state.clone(),
                                );
                            } else {
                                creators.insert(
                                    (schema.clone(), member),
                                    vec![rol.namespace.clone()],
                                );
                            }
                        }
                    }
                }
                _ => {}
            };
        }
        (our_roles, creators)
    }

    pub fn get_schemas(&self) -> Vec<Schema> {
        self.schemas.clone()
    }

    /// Check if the request is allowed.
    pub fn is_allowed(
        &self,
        id: KeyIdentifier,
        name: &str,
        stage: RequestStage,
    ) -> bool {
        for rol in &self.roles {
            if rol.role.to_str() == stage.to_role() {
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
        self.version
    }

    /// Check if the key is a member.
    fn is_member(&self, id: &KeyIdentifier) -> bool {
        for member in &self.members {
            if member.id == id.to_string() {
                return true;
            }
        }
        false
    }
}

impl TryFrom<SubjectState> for Governance {
    type Error = Error;

    fn try_from(subject: SubjectState) -> Result<Self, Self::Error> {
        let governance: Governance =
            serde_json::from_value(subject.properties.0).map_err(|_| {
                Error::Governance("Governance model not found.".to_owned())
            })?;
        Ok(governance)
    }
}
