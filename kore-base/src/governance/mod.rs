// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance module.
//!

pub mod init;
pub mod json_schema;
pub mod model;
mod schema;

use crate::{
    model::{wrapper::ValueWrapper, Namespace},
    Error,
};

use actor::Error as ActorError;
use model::{CreatorQuantity, Roles};
pub use schema::schema;

pub use model::{Member, Policy, Quorum, RequestStage, Role, Schema, Who};

use identity::identifier::KeyIdentifier;

use serde::{Deserialize, Serialize};

use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

type SchemRolNameSpace = HashMap<(String, Roles), Vec<String>>;
type SchemaKeyNameSpace = HashMap<(String, String), Vec<String>>;

/// Governance struct.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Governance {
    pub version: u64,
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
                return Ok(ValueWrapper(schema.initial_value.clone()));
            }
        }

        Err(Error::Governance("Schema not found.".to_owned()))
    }

    /// Get the schema by id.
    ///
    pub fn get_schema(&self, schema_id: &str) -> Result<Schema, Error> {
        for schema in &self.schemas {
            if schema.id == schema_id {
                return Ok(schema.clone());
            }
        }
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

    pub fn has_this_role(
        &self,
        user: &str,
        role: Roles,
        schema: &str,
        namespace: Namespace,
    ) -> bool {
        for rol in &self.roles {
            if role == rol.role {
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
                        return true;
                    }
                    Who::ID { ID } => {
                        if user == ID {
                            return true;
                        }
                    }

                    Who::NAME { NAME } => {
                        let id_string = self.id_by_name(&NAME);
                        if let Some(id) = id_string {
                            if user == id {
                                return true;
                            }
                        }
                    }
                    Who::NOT_MEMBERS => {
                        unreachable!()
                    }
                }
            }
        }
        false
    }

    pub fn max_creations(
        &self,
        user: &str,
        schema: &str,
        namespace: Namespace,
    ) -> Option<CreatorQuantity> {
        for rol in &self.roles {
            if let Roles::CREATOR(quantity) = rol.role.clone() {
                let namespace_role = Namespace::from(rol.namespace.to_string());
                if namespace_role != namespace {
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
                        return Some(quantity);
                    }
                    Who::ID { ID } => {
                        if user == ID {
                            return Some(quantity);
                        }
                    }

                    Who::NAME { NAME } => {
                        let id_string = self.id_by_name(&NAME);
                        if let Some(id) = id_string {
                            if user == id {
                                return Some(quantity);
                            }
                        }
                    }
                    Who::NOT_MEMBERS => {
                        unreachable!()
                    }
                }
            }
        }
        None
    }

    /// Gets the signers for the request stage.
    pub fn get_signers(
        &self,
        role: Roles,
        schema: &str,
        namespace: Namespace,
    ) -> (HashSet<KeyIdentifier>, bool) {
        let mut signers = HashSet::new();
        let mut not_members = false;

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
                        return (signers, not_members);
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
                        not_members = true;
                    }
                }
            }
        }

        (signers, not_members)
    }

    fn get_quorum(&self, role: Roles, schema: &str) -> Option<Quorum> {
        let policies = self.policies.iter().find(|e| e.id == schema);
        if let Some(policies) = policies {
            match role {
                Roles::APPROVER => Some(policies.approve.quorum.clone()),
                Roles::EVALUATOR => Some(policies.evaluate.quorum.clone()),
                Roles::VALIDATOR => Some(policies.validate.quorum.clone()),
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn get_quorum_and_signers(
        &self,
        role: Roles,
        schema: &str,
        namespace: Namespace,
    ) -> Result<(HashSet<KeyIdentifier>, Quorum), ActorError> {
        let (signers, _not_members) =
            self.get_signers(role.clone(), schema, namespace);
        let Some(quorum) = self.get_quorum(role.clone(), schema) else {
            return Err(ActorError::Functional(format!(
                "No quorum found for role {} and schema {}",
                role, schema
            )));
        };

        Ok((signers, quorum))
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
            self.schemas.clone()
        } else {
            let mut schemas: Vec<Schema> = vec![];
            for id in schemas_id {
                for schema in self.schemas.clone() {
                    if id == schema.id {
                        schemas.push(schema);
                    }
                }
            }

            schemas
        }
    }

    pub fn subjects_schemas_rol_namespace(
        &self,
        our_id: &str,
    ) -> (SchemRolNameSpace, SchemaKeyNameSpace) {
        let mut our_roles: SchemRolNameSpace = HashMap::new();
        let mut creators: SchemaKeyNameSpace = HashMap::new();
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
                Roles::EVALUATOR | Roles::VALIDATOR => {
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
                Roles::CREATOR { .. } => {
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

impl TryFrom<ValueWrapper> for Governance {
    type Error = Error;

    fn try_from(value: ValueWrapper) -> Result<Self, Self::Error> {
        let governance: Governance =
            serde_json::from_value(value.0).map_err(|_| {
                Error::Governance("Governance model not found.".to_owned())
            })?;
        Ok(governance)
    }
}
