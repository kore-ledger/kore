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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Governance {
    /// The version.
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
    /// Get the initial state for governance model
    ///  # Arguments
    ///  * `schema_id` - The identifier of the [`Schema`].
    /// # Returns
    /// * [`ValueWrapper`] - The initial state.
    /// # Errors
    /// * `Error` - If the schema is not found.
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
    /// # Arguments
    /// * `schema_id` - The identifier of the [`Schema`].
    /// # Returns
    /// * [`Schema`] - The schema.
    pub fn get_schema(&self, schema_id: &str) -> Result<Schema, Error> {
        for schema in &self.schemas {
            if schema.id == schema_id {
                return Ok(schema.clone());
            }
        }
        Err(Error::Governance("Schema not found.".to_owned()))
    }

    /// Get the members as a set of key identifiers.
    /// # Returns
    /// * `HashSet<KeyIdentifier>` - The set of key [`KeyIdentifier`].
    /// # Errors
    /// * `Error` - If the key identifier is not valid.
    pub fn members_to_key_identifier(&self) -> HashSet<KeyIdentifier> {
        HashSet::from_iter(
            self.members
                .iter()
                .filter_map(|e| KeyIdentifier::from_str(&e.id).ok()),
        )
    }

    /// Get the member id by name.
    /// # Arguments
    /// * `name` - The name of the member.
    /// # Returns
    /// * `Option<String>` - The member id.
    fn id_by_name(&self, name: &str) -> Option<String> {
        let member = self.members.iter().find(|e| e.name == name);
        member.map(|member| member.id.clone())
    }

    /// Check if the user has a role.
    /// # Arguments
    /// * `user` - The user id.
    /// * [`Roles`] - The role.
    /// * `schema` - The schema id from [`Schema`].
    /// * [`Namespace`] - The namespace.
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
                        if self.is_member(user) {
                            return true;
                        }
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

    /// Get the maximum creations for the user.
    /// # Arguments
    /// * `user` - The user id.
    /// * `schema` - The schema id from [`Schema`].
    /// * [`Namespace`] - The namespace.
    /// # Returns
    /// * Option<[`CreatorQuantity`]> - The maximum creations.
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
    /// # Arguments
    /// * [`Roles`] - The role.
    /// * `schema` - The schema id from [`Schema`].
    /// * [`Namespace`] - The namespace.
    /// # Returns
    /// * (HashSet<[`KeyIdentifier`]>, bool) - The set of key identifiers and a flag indicating if the user is not a member.
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

    /// Get the quorum for the role and schema.
    /// # Arguments
    /// * [`Roles`] - The role.
    /// * `schema` - The schema id from [`Schema`].
    /// # Returns
    /// * Option<[`Quorum`]> - The quorum.
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

    /// Get the quorum and signers for the role and schema.
    /// # Arguments
    /// * [`Roles`] - The role.
    /// * `schema` - The schema id from [`Schema`].
    /// * [`Namespace`] - The namespace.
    /// # Returns
    /// * (HashSet<[`KeyIdentifier`]>, [`Quorum`]) - The set of key identifiers and the quorum.
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

    /// Get the schemas for the role and user.
    /// # Arguments
    /// * [`Roles`] - The role.
    /// * `our_id` - The user id.
    /// # Returns
    /// * Vec<[`Schema`]> - The schemas.
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

    /// Get the signers for the role and schema.
    /// # Arguments
    /// * `our_id` - The user id.
    /// # Returns
    /// ([`SchemRolNameSpace`],[`SchemaKeyNameSpace`]) - The roles and creators.
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

    /// Check if the key is a member.
    fn is_member(&self, id: &str) -> bool {
        for member in &self.members {
            if member.id == id {
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
            serde_json::from_value(value.0).map_err(|e| {
                Error::Governance(format!(
                    "Can not convert Value into Governance: {}",
                    e
                ))
            })?;
        Ok(governance)
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{Namespace, ValueWrapper};

    use super::{
        init::init_state,
        model::{Contract, CreatorQuantity, Roles, SchemaEnum, Validation},
        Governance, Member, Policy, Role, Schema, Who,
    };
    use identity::{
        identifier::{Derivable, KeyIdentifier},
        keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
    };
    use serde_json::Value;

    fn create_governance(
        key1: KeyIdentifier,
        key2: KeyIdentifier,
    ) -> Governance {
        Governance {
            version: 1,
            members: vec![
                Member {
                    id: key1.to_string(),
                    name: "test".to_string(),
                    description: Some("".to_string()),
                },
                Member {
                    id: key2.to_string(),
                    name: "test1".to_string(),
                    description: Some("".to_string()),
                },
            ],
            roles: vec![
                Role {
                    who: Who::NAME {
                        NAME: "test".to_string(),
                    },
                    namespace: "".to_string(),
                    role: Roles::CREATOR(CreatorQuantity::QUANTITY(12)),
                    schema: SchemaEnum::ALL,
                },
                Role {
                    who: Who::NAME {
                        NAME: "test1".to_string(),
                    },
                    namespace: "".to_string(),
                    role: Roles::CREATOR(CreatorQuantity::INFINITY),
                    schema: SchemaEnum::ALL,
                },
                Role {
                    who: Who::NAME {
                        NAME: "test1".to_string(),
                    },
                    namespace: "".to_string(),
                    role: Roles::APPROVER,
                    schema: SchemaEnum::ID {
                        ID: "governance".to_string(),
                    },
                },
            ],
            schemas: vec![Schema {
                id: "governance".to_string(),
                initial_value: Value::default(),
                contract: Contract {
                    raw: "".to_string(),
                },
            }],
            policies: vec![Policy {
                id: "governance".to_string(),
                approve: Validation {
                    quorum: super::Quorum::MAJORITY,
                },
                evaluate: Validation {
                    quorum: super::Quorum::MAJORITY,
                },
                validate: Validation {
                    quorum: super::Quorum::MAJORITY,
                },
            }],
        }
    }

    fn create_governance_init_state() -> Governance {
        let keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let governance_value = init_state(&keys.key_identifier().to_string());
        Governance::try_from(governance_value).unwrap()
    }
    #[tokio::test]
    async fn test_try_from_value_wrapper() {
        let original_gov = create_governance_init_state();
        let wrapper =
            ValueWrapper(serde_json::to_value(&original_gov).unwrap());
        let new_gov = Governance::try_from(wrapper);
        assert!(new_gov.is_ok());

        let invalid_wrapper = ValueWrapper(serde_json::json!({ "abc": 123 }));
        let invalid_gov = Governance::try_from(invalid_wrapper);
        assert!(invalid_gov.is_err());
    }

    #[tokio::test]
    async fn test_init_state() {
        let gov = create_governance_init_state();
        assert_eq!(gov.version, 0);
        assert!(!gov.members.is_empty());
        assert!(!gov.roles.is_empty());
        assert!(gov.schemas.is_empty());
        assert!(!gov.policies.is_empty());
    }

    #[tokio::test]
    async fn test_get_schema() {
        let gov = create_governance_init_state();
        if gov.schemas.is_empty() {
            let res = gov.get_schema("governance");
            assert!(res.is_err());
        } else {
            panic!("")
        }
        let fail = gov.get_schema("missing_schema");
        assert!(fail.is_err());
    }

    #[tokio::test]
    async fn test_members_to_key_identifier() {
        let key1 = KeyPair::Ed25519(Ed25519KeyPair::new());
        let key2 = KeyPair::Ed25519(Ed25519KeyPair::new());

        let gov =
            create_governance(key1.key_identifier(), key2.key_identifier());
        let set_ids = gov.members_to_key_identifier();

        assert_eq!(set_ids.len(), 2);
        assert!(set_ids.contains(&key1.key_identifier()));
        assert!(set_ids.contains(&key2.key_identifier()));
    }

    #[tokio::test]
    async fn test_max_creation() {
        let key1 = KeyPair::Ed25519(Ed25519KeyPair::new());
        let key2 = KeyPair::Ed25519(Ed25519KeyPair::new());
        let governance =
            create_governance(key1.key_identifier(), key2.key_identifier());
        let response = governance
            .max_creations(
                &key1.key_identifier().to_str(),
                "governance",
                Namespace::new(),
            )
            .unwrap();
        assert_eq!(response, CreatorQuantity::QUANTITY(12));
        let response = governance
            .max_creations(
                &key2.key_identifier().to_str(),
                "governance",
                Namespace::new(),
            )
            .unwrap();
        assert_eq!(response, CreatorQuantity::INFINITY)
    }

    #[tokio::test]
    async fn test_get_signers() {
        let key1 = KeyPair::Ed25519(Ed25519KeyPair::new());
        let key2 = KeyPair::Ed25519(Ed25519KeyPair::new());
        let governance =
            create_governance(key1.key_identifier(), key2.key_identifier());
        let response = governance.get_signers(
            Roles::CREATOR(CreatorQuantity::QUANTITY(12)),
            "governance",
            Namespace::new(),
        );
        response.0.get(&key1.key_identifier()).unwrap();
        assert!(!response.1);
        let response = governance.get_signers(
            Roles::CREATOR(CreatorQuantity::INFINITY),
            "governance",
            Namespace::new(),
        );
        response.0.get(&key2.key_identifier()).unwrap();
        assert!(!response.1);
        let random_key = KeyPair::Ed25519(Ed25519KeyPair::new());
        let response3 = governance.max_creations(
            &random_key.key_identifier().to_string(),
            "governance",
            Namespace::new(),
        );
        assert!(response3.is_none());
    }
    #[tokio::test]
    async fn test_has_this_role() {
        let key1 = KeyPair::Ed25519(Ed25519KeyPair::new());
        let governance =
            create_governance(key1.key_identifier(), KeyIdentifier::default());

        // El usuario `key1` tiene el rol de CREATOR(12) (nombre "test")
        let has_role = governance.has_this_role(
            &key1.key_identifier().to_string(),
            Roles::CREATOR(CreatorQuantity::QUANTITY(12)),
            "All",
            Namespace::new(),
        );
        assert!(has_role);

        // No debería tener el rol de APPROVER
        let has_role_approver = governance.has_this_role(
            &key1.key_identifier().to_string(),
            Roles::APPROVER,
            "All",
            Namespace::new(),
        );
        assert!(!has_role_approver);
    }

    #[tokio::test]
    async fn test_get_quorum_and_signers() {
        let key1 = KeyPair::Ed25519(Ed25519KeyPair::new());
        let gov =
            create_governance(key1.key_identifier(), KeyIdentifier::default());

        // En create_governance, tenemos `policies` con Quorum::MAJORITY para governance
        // Rol: APPROVER y schema: "governance" => debe existir
        let res = gov.get_quorum_and_signers(
            Roles::APPROVER,
            "governance",
            Namespace::new(),
        );
        assert!(
            res.is_ok(),
            "Debería existir la policy para APPROVER-governance"
        );
        let (signers, _) = res.unwrap();
        // signers debe contener a test1 (quien tiene APPROVER en "governance"):
        assert!(
            signers.is_empty() == false,
            "En create_governance se asignó APPROVER a test1"
        );

        // Caso error -> schema que no exista
        let error_res = gov.get_quorum_and_signers(
            Roles::APPROVER,
            "missing",
            Namespace::new(),
        );
        assert!(error_res.is_err(), "No existe la policy para esa schema");
    }

    #[tokio::test]
    async fn test_schemas_method() {
        let key1 = KeyPair::Ed25519(Ed25519KeyPair::new());
        let governance =
            create_governance(key1.key_identifier(), KeyIdentifier::default());

        // Este key1 tiene Roles::CREATOR(12) para todos los schemas
        let schemas = governance.schemas(
            Roles::CREATOR(CreatorQuantity::QUANTITY(12)),
            &key1.key_identifier().to_string(),
        );
        // Debería retornar TODOS los schemas (solo hay uno: "governance")
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0].id, "governance");

        // Usuario sin rol no obtiene nada
        let random_key = KeyPair::Ed25519(Ed25519KeyPair::new());
        let schemas_empty = governance.schemas(
            Roles::CREATOR(CreatorQuantity::QUANTITY(12)),
            &random_key.key_identifier().to_string(),
        );
        assert_eq!(schemas_empty.len(), 0);
    }

    #[tokio::test]
    async fn test_subjects_schemas_rol_namespace() {
        let key1 = KeyPair::Ed25519(Ed25519KeyPair::new());
        let key2 = KeyPair::Ed25519(Ed25519KeyPair::new());
        let governance =
            create_governance(key1.key_identifier(), key2.key_identifier());

        let (roles_map, creators_map) = governance
            .subjects_schemas_rol_namespace(&key1.key_identifier().to_string());

        // roles_map -> Rol EVALUATOR o VALIDATOR, etc. En nuestra create_governance no hay EVALUATOR/VALIDATOR,
        // así que debería estar vacío
        assert!(roles_map.is_empty());
        println!("{:?}", creators_map);
        assert!(!creators_map.is_empty());
    }
}
