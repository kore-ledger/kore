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
    RoleSchemaIssuer, RoleTypes, RolesAllSchemas, RolesGov, RolesSchema,
    SchemaKeyCreators,
};

pub use model::{Member, Quorum, Role, Schema};

use identity::identifier::KeyIdentifier;

use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, BTreeSet, HashSet};

pub type MemberName = String;
pub type SchemaId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Governance {
    pub version: u64,
    pub members: BTreeMap<MemberName, KeyIdentifier>,
    pub roles_gov: RolesGov,
    pub policies_gov: PolicyGov,
    pub schemas: BTreeMap<SchemaId, Schema>,
    pub roles_schema: BTreeMap<SchemaId, RolesSchema>,
    pub roles_all_schemas: RolesAllSchemas,
    pub policies_schema: BTreeMap<SchemaId, PolicySchema>,
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
            self.roles_schema
                .insert(schema.clone(), RolesSchema::default());
            self.policies_schema.insert(schema, PolicySchema::default());
        }
    }

    pub fn remove_member_role(&mut self, remove_members: &Vec<MemberName>) {
        self.roles_gov.remove_member_role(remove_members);
        self.roles_all_schemas.remove_member_role(remove_members);

        for (_, roles) in self.roles_schema.iter_mut() {
            roles.remove_member_role(remove_members);
        }
    }

    pub fn change_name_role(
        &mut self,
        chang_name_members: &Vec<(String, String)>,
    ) {
        self.roles_gov.change_name_role(chang_name_members);
        self.roles_all_schemas.change_name_role(chang_name_members);

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

    pub fn check_basic_gov(&self) -> bool {
        self.roles_gov.check_basic_gov()
            && self.roles_all_schemas.check_basic_gov()
    }

    pub fn new(owner_key: KeyIdentifier) -> Self {
        let policies_gov = PolicyGov {
            approve: Quorum::Majority,
            evaluate: Quorum::Majority,
            validate: Quorum::Majority,
        };

        let owner_users_schema: BTreeSet<Role> = BTreeSet::from([Role {
            name: "Owner".to_owned(),
            namespace: Namespace::new(),
        }]);
        let owner_users_gov: BTreeSet<MemberName> =
            BTreeSet::from(["Owner".to_owned()]);

        let roles_gov = RolesGov {
            approver: owner_users_gov.clone(),
            evaluator: owner_users_gov.clone(),
            validator: owner_users_gov.clone(),
            witness: BTreeSet::new(),
            issuer: RoleGovIssuer {
                any: false,
                users: owner_users_gov.clone(),
            },
        };

        let not_gov_role = RolesAllSchemas {
            evaluator: owner_users_schema.clone(),
            validator: owner_users_schema.clone(),
            witness: owner_users_schema,
            issuer: RoleSchemaIssuer {
                users: BTreeSet::new(),
                any: false,
            },
        };

        Self {
            version: 0,
            members: BTreeMap::from([("Owner".to_owned(), owner_key)]),
            roles_gov,
            policies_gov,
            schemas: BTreeMap::new(),
            roles_schema: BTreeMap::new(),
            roles_all_schemas: not_gov_role,
            policies_schema: BTreeMap::new(),
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

        Ok(ValueWrapper(schema.initial_value.clone()))
    }

    /// Get the members as a set of key identifiers.
    /// # Returns
    /// * `HashSet<KeyIdentifier>` - The set of key [`KeyIdentifier`].
    /// # Errors
    /// * `Error` - If the key identifier is not valid.
    pub fn members_to_key_identifier(&self) -> HashSet<KeyIdentifier> {
        HashSet::from_iter(self.members.values().cloned())
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
            if let RoleTypes::Witness = role {
                return true;
            }

            self.roles_gov.hash_this_rol(role, &name)
        } else {
            if self.roles_all_schemas.hash_this_rol(
                role.clone(),
                namespace.clone(),
                &name,
            ) {
                return true;
            }

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
        key: &KeyIdentifier,
        schema: &str,
        namespace: Namespace,
    ) -> Option<CreatorQuantity> {
        let name = self
            .members
            .iter()
            .find(|x| x.1 == key)
            .map(|x| x.0)
            .cloned()?;

        let roles = self.roles_schema.get(schema)?;

        roles.max_creations(namespace, &name)
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
            let (mut not_gov_signers, not_gov_any) = self
                .roles_all_schemas
                .get_signers(role.clone(), namespace.clone());
            let (mut schema_signers, schema_any) =
                if let Some(roles) = self.roles_schema.get(schema) {
                    roles.get_signers(role, namespace)
                } else {
                    (vec![], false)
                };

            not_gov_signers.append(&mut schema_signers);

            (not_gov_signers, not_gov_any || schema_any)
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
            let policie = self.policies_schema.get(schema)?;

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
    ) -> BTreeMap<SchemaId, Schema> {
        let Some(name) = self
            .members
            .iter()
            .find(|x| x.1 == key)
            .map(|x| x.0)
            .cloned()
        else {
            return BTreeMap::new();
        };
        let role = RoleTypes::from(role);

        if self
            .roles_all_schemas
            .hash_this_rol_not_namespace(role.clone(), &name)
        {
            return self.schemas.clone();
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

        copy_schemas
    }

    pub fn subjects_schemas_rol_namespace(
        &self,
        key: &KeyIdentifier,
    ) -> Vec<SchemaKeyCreators> {
        let Some(name) = self
            .members
            .iter()
            .find(|x| x.1 == key)
            .map(|x| x.0)
            .cloned()
        else {
            return vec![];
        };

        let (not_gov_val, not_gov_eval) =
            self.roles_all_schemas.roles_namespace(&name);

        let mut schema_key_creators: Vec<SchemaKeyCreators> = vec![];

        for (schema, roles) in self.roles_schema.iter() {
            let schema_creators = roles.roles_creators(
                &name,
                not_gov_val.clone(),
                not_gov_eval.clone(),
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
                            return vec![];
                        };
                        hash_keys.insert(key.clone());
                    }

                    schema_key.validation = Some(hash_keys);
                }

                if let Some(eval_schema_creators) = schema_creators.evaluation {
                    let mut hash_keys: HashSet<KeyIdentifier> = HashSet::new();
                    for name in eval_schema_creators {
                        let Some(key) = self.members.get(&name) else {
                            return vec![];
                        };
                        hash_keys.insert(key.clone());
                    }
                    schema_key.evaluation = Some(hash_keys);
                }

                schema_key_creators.push(schema_key);
            }
        }
        schema_key_creators
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
