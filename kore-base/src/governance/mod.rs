// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance module.
//!

pub mod events;
pub mod model;

use crate::{
    Error,
    model::{Namespace, wrapper::ValueWrapper},
};

use actor::Error as ActorError;
use model::{
    CreatorQuantity, PolicyGov, PolicySchema, ProtocolTypes, RoleGovIssuer,
    RoleSchemaIssuer, RoleTypes, RolesGov, RolesSchema, SchemaKeyCreators,
};

pub use model::{Member, Quorum, Role, Schema};

use identity::identifier::KeyIdentifier;

use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet};

pub type MemberName = String;
pub type SchemaId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Governance {
    pub version: u64,
    pub members: HashMap<MemberName, KeyIdentifier>,
    pub roles_gov: RolesGov,
    pub policies_gov: PolicyGov,
    pub schemas: HashMap<SchemaId, Schema>,
    pub roles_schema: HashMap<SchemaId, RolesSchema>,
    pub roles_not_governance: RolesSchema,
    pub policies_schema: HashMap<SchemaId, PolicySchema>,
}

impl Governance {
    pub fn remove_schema(&mut self, remove_schemas: HashSet<SchemaId>) {
        for schema in remove_schemas {
            self.roles_schema.remove(&schema);
            self.policies_schema.remove(&schema);
        }
    }

    pub fn add_schema(&mut self, add_schema: HashSet<SchemaId>) {
        for schema in add_schema {
            self.roles_schema.insert(schema.clone(), RolesSchema::default());
            self.policies_schema.insert(schema, PolicySchema::default());
        }
    }

    pub fn remove_member_role(&mut self, remove_members: &Vec<MemberName>) {
        self.roles_gov.remove_member_role(remove_members);

        for (_, roles) in self.roles_schema.iter_mut() {
            roles.remove_member_role(remove_members);
        }
    }

    pub fn change_name_role(
        &mut self,
        chang_name_members: &Vec<(String, String)>,
    ) {
        self.roles_gov.change_name_role(chang_name_members);

        for (_, roles) in self.roles_schema.iter_mut() {
            roles.change_name_role(chang_name_members);
        }
    }

    pub fn to_value_wrapper(&self) -> Result<ValueWrapper, Error> {
        Ok(ValueWrapper(serde_json::to_value(self).map_err(|e| {
            Error::Governance(format!(
                "Can not convert governance into Value: {}",
                e
            ))
        })?))
    }

    pub fn new(owner_key: KeyIdentifier) -> Self {
        let policies_gov = PolicyGov {
            approve: Quorum::Majority,
            evaluate: Quorum::Majority,
            validate: Quorum::Majority,
        };

        let owner_users_schema: HashSet<Role> = HashSet::from([Role {
            name: "Owner".to_owned(),
            namespace: Namespace::new(),
        }]);
        let owner_users_gov: HashSet<MemberName> =
            HashSet::from(["Owner".to_owned()]);

        let roles_gov = RolesGov {
            approver: owner_users_gov.clone(),
            evaluator: owner_users_gov.clone(),
            validator: owner_users_gov.clone(),
            witness: HashSet::new(),
            issuer: RoleGovIssuer {
                any: false,
                users: owner_users_gov.clone(),
            },
        };

        let not_gov_role = RolesSchema {
            evaluator: owner_users_schema.clone(),
            validator: owner_users_schema.clone(),
            witness: owner_users_schema,
            creator: HashSet::new(),
            issuer: RoleSchemaIssuer {
                users: HashSet::new(),
                any: false,
            },
        };

        Self {
            version: 0,
            members: HashMap::from([("Owner".to_owned(), owner_key)]),
            roles_gov,
            policies_gov,
            schemas: HashMap::new(),
            roles_schema: HashMap::new(),
            roles_not_governance: not_gov_role,
            policies_schema: HashMap::new(),
        }
    }

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
        let Some(schema) = self.schemas.get(schema_id) else {
            return Err(Error::Governance("Schema not found.".to_owned()));
        };

        return Ok(ValueWrapper(schema.initial_value.clone()));
    }

    /// Get the members as a set of key identifiers.
    /// # Returns
    /// * `HashSet<KeyIdentifier>` - The set of key [`KeyIdentifier`].
    /// # Errors
    /// * `Error` - If the key identifier is not valid.
    pub fn members_to_key_identifier(&self) -> HashSet<KeyIdentifier> {
        HashSet::from_iter(self.members.iter().map(|(_name, key)| key.clone()))
    }

    /// Check if the user has a role.
    /// # Arguments
    /// * `user` - The user id.
    /// * [`Roles`] - The role.
    /// * `schema` - The schema id from [`Schema`].
    /// * [`Namespace`] - The namespace.
    pub fn has_this_role(
        &self,
        key: &KeyIdentifier,
        role: RoleTypes,
        schema: &str,
        namespace: Namespace,
    ) -> bool {
        let Some(name) = self
            .members
            .iter()
            .find(|x| x.1 == key)
            .map(|x| x.0)
            .cloned()
        else {
            return false;
        };

        if schema == "governance" {
            self.roles_gov.hash_this_rol(role, &name)
        } else {
            let Some(roles) = self.roles_schema.get(schema) else {
                return false;
            };

            roles.hash_this_rol(role, namespace, &name)
        }
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
        name: &str,
        schema: &str,
        namespace: Namespace,
    ) -> Option<CreatorQuantity> {
        let Some(roles) = self.roles_schema.get(schema) else {
            return None;
        };

        roles.max_creations(namespace, name)
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
        role: RoleTypes,
        schema: &str,
        namespace: Namespace,
    ) -> (HashSet<KeyIdentifier>, bool) {
        let (names, any) = if schema == "governance" {
            self.roles_gov.get_signers(role)
        } else {
            let Some(roles) = self.roles_schema.get(schema) else {
                return (HashSet::new(), false);
            };
            roles.get_signers(role, namespace)
        };

        let mut signers = HashSet::new();
        for name in names {
            if let Some(key) = self.members.get(&name) {
                signers.insert(key.clone());
            }
        }

        (signers, any)
    }

    /// Get the quorum for the role and schema.
    /// # Arguments
    /// * [`Roles`] - The role.
    /// * `schema` - The schema id from [`Schema`].
    /// # Returns
    /// * Option<[`Quorum`]> - The quorum.
    fn get_quorum(&self, role: ProtocolTypes, schema: &str) -> Option<Quorum> {
        if schema == "governance" {
            self.policies_gov.get_quorum(role)
        } else {
            let Some(policie) = self.policies_schema.get(schema) else {
                return None;
            };

            policie.get_quorum(role)
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
        role: ProtocolTypes,
        schema: &str,
        namespace: Namespace,
    ) -> Result<(HashSet<KeyIdentifier>, Quorum), ActorError> {
        let (signers, _not_members) =
            self.get_signers(RoleTypes::from(role.clone()), schema, namespace);

        let Some(quorum) = self.get_quorum(role.clone(), schema) else {
            return Err(ActorError::Functional(format!(
                "No quorum found for role {} and schema {}",
                role, schema
            )));
        };

        Ok((signers, quorum))
    }

    pub fn schemas(
        &self,
        role: ProtocolTypes,
        key: &KeyIdentifier,
    ) -> Result<HashMap<SchemaId, Schema>, Error> {
        let Some(name) = self
            .members
            .iter()
            .find(|x| x.1 == key)
            .map(|x| x.0)
            .cloned()
        else {
            return Err(Error::Governance(format!(
                "Can not get member by KeyIdentifier: {}",
                key
            )));
        };
        let role = RoleTypes::from(role);

        if self
            .roles_not_governance
            .hash_this_rol_not_namespace(role.clone(), &name)
        {
            return Ok(self.schemas.clone());
        }

        let mut not_schemas: Vec<String> = vec![];

        for (schema, roles) in self.roles_schema.iter() {
            if !roles.hash_this_rol_not_namespace(role.clone(), &name) {
                not_schemas.push(schema.clone());
            }
        }

        let mut copy_schemas = self.schemas.clone();
        for schema in not_schemas {
            copy_schemas.remove(&schema);
        }

        Ok(copy_schemas)
    }

    pub fn subjects_schemas_rol_namespace(
        &self,
        key: &KeyIdentifier,
    ) -> Result<Vec<SchemaKeyCreators>, Error> {
        let Some(name) = self
            .members
            .iter()
            .find(|x| x.1 == key)
            .map(|x| x.0)
            .cloned()
        else {
            return Err(Error::Governance(format!(
                "Can not get member by KeyIdentifier: {}",
                key
            )));
        };

        let (not_gov_val, not_gov_eval, not_gov_creators) =
            self.roles_not_governance.roles_namespace_creators(&name);

        let mut schema_key_creators: Vec<SchemaKeyCreators> = vec![];

        for (schema, roles) in self.roles_schema.iter() {
            let schema_creators = roles.roles_creators(
                &name,
                not_gov_val.clone(),
                not_gov_eval.clone(),
                not_gov_creators.clone(),
            );
            if !schema_creators.is_empty() {
                let mut schema_key = SchemaKeyCreators {
                    schema: schema.clone(),
                    validation: None,
                    evaluation: None,
                };

                if let Some(val_schema_creators) = schema_creators.validation {
                    let mut hash_keys: HashSet<KeyIdentifier> = HashSet::new();
                    for name in val_schema_creators {
                        let Some(key) = self.members.get(&name) else {
                            return Err(Error::Governance(format!(
                                "Can not find KeyIdentifier of: {}",
                                name
                            )));
                        };
                        hash_keys.insert(key.clone());
                    }

                    schema_key.validation = Some(hash_keys);
                }

                if let Some(eval_schema_creators) = schema_creators.evaluation {
                    let mut hash_keys: HashSet<KeyIdentifier> = HashSet::new();
                    for name in eval_schema_creators {
                        let Some(key) = self.members.get(&name) else {
                            return Err(Error::Governance(format!(
                                "Can not find KeyIdentifier of: {}",
                                name
                            )));
                        };
                        hash_keys.insert(key.clone());
                    }
                    schema_key.evaluation = Some(hash_keys);
                }

                schema_key_creators.push(schema_key);
            }
        }
        Ok(schema_key_creators)
    }

    /// Check if the key is a member.
    pub fn is_member(&self, key: &KeyIdentifier) -> bool {
        self.members.iter().any(|x| x.1 == key)
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
    /*
    use crate::model::{Namespace, ValueWrapper};
    use test_log::test;

    use super::{
        Governance, Member, Policy, Role, Schema, Who,
        init::init_state,
        model::{Contract, CreatorQuantity, Roles, SchemaEnum, Validation},
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
                },
                Member {
                    id: key2.to_string(),
                    name: "test1".to_string(),
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
    #[test(tokio::test)]
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

    #[test(tokio::test)]
    async fn test_init_state() {
        let gov = create_governance_init_state();
        assert_eq!(gov.version, 0);
        assert!(!gov.members.is_empty());
        assert!(!gov.roles.is_empty());
        assert!(gov.schemas.is_empty());
        assert!(!gov.policies.is_empty());
    }

    #[test(tokio::test)]
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

    #[test(tokio::test)]
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

    #[test(tokio::test)]
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

    #[test(tokio::test)]
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
    #[test(tokio::test)]
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

    #[test(tokio::test)]
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

    #[test(tokio::test)]
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

    #[test(tokio::test)]
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
     */
}
