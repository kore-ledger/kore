// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance model.
//!

use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};

use std::{
    collections::HashSet,
    fmt::{self}, hash::Hash,
};

use crate::model::Namespace;
pub type MemberName = String;

/// Governance schema.
#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct Schema {
    pub initial_value: serde_json::Value,
    pub contract: String,
}

pub struct NameCreators {
    pub validation: Option<HashSet<String>>,
    pub evaluation: Option<HashSet<String>>
}

impl NameCreators {
    pub fn is_empty(&self) -> bool {
        self.validation.is_none() && self.evaluation.is_none()
    }
}

pub struct SchemaKeyCreators {
    pub schema: String,
    pub validation: Option<HashSet<KeyIdentifier>>,
    pub evaluation: Option<HashSet<KeyIdentifier>>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RolesGov {
    pub approver: HashSet<MemberName>,
    pub evaluator: HashSet<MemberName>,
    pub validator: HashSet<MemberName>,
    pub witness: HashSet<MemberName>,
    pub issuer: RoleGovIssuer,
}

impl RolesGov {
    pub fn remove_member_role(&mut self, remove_members: &Vec<String>) {
        for remove in remove_members {
            self.approver.remove(remove);
            self.evaluator.remove(remove);
            self.validator.remove(remove);
            self.witness.remove(remove);
            self.issuer.users.remove(remove);
        }   
    }

    pub fn change_name_role(&mut self, chang_name_members: &Vec<(String, String)>) {
        for (old_name, new_name) in chang_name_members {
            if self.approver.remove(old_name) {
                self.approver.insert(new_name.clone());
            };
            if self.evaluator.remove(old_name) {
                self.evaluator.insert(new_name.clone());
            };
            if self.validator.remove(old_name) {
                self.validator.insert(new_name.clone());
            };
            if self.witness.remove(old_name) {
                self.witness.insert(new_name.clone());
            };
            if self.issuer.users.remove(old_name) {
                self.issuer.users.insert(new_name.clone());
            };            
        }
    }

    pub fn hash_this_rol(
        &self,
        role: RoleTypes,
        name: &str,
    ) -> bool {
        match role {
            RoleTypes::Approver => self.approver.get(name).is_some(),
            RoleTypes::Evaluator => self.evaluator.get(name).is_some(),
            RoleTypes::Validator => self.validator.get(name).is_some(),
            RoleTypes::Issuer => self.issuer.users.get(name).is_some() || self.issuer.any,
            RoleTypes::Creator => self.witness.get(name).is_some(),
            RoleTypes::Witness => false
        }
    }

    pub fn get_signers(
        &self,
        role: RoleTypes,
    ) -> (Vec<String>, bool) {
        match role {
            RoleTypes::Evaluator => (
                self.evaluator
                    .iter()
                    .map(|x| x.clone())
                    .collect::<Vec<String>>(),
                false,
            ),
            RoleTypes::Validator => (
                self.validator
                    .iter()
                    .map(|x| x.clone())
                    .collect::<Vec<String>>(),
                false,
            ),
            RoleTypes::Approver => (
                self.approver
                    .iter()
                    .map(|x| x.clone())
                    .collect::<Vec<String>>(),
                false,
            ),
            RoleTypes::Issuer => (
                self.issuer
                    .users
                    .iter()
                    .map(|x| x.clone())
                    .collect::<Vec<String>>(),
                self.issuer.any,
            ),
            RoleTypes::Witness => (
                self.witness            
                    .iter()
                    .map(|x| x.clone())
                    .collect::<Vec<String>>(),
                false
            ),
            RoleTypes::Creator => (vec![], false)
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct RolesSchema {
    pub evaluator: HashSet<Role>,
    pub validator: HashSet<Role>,
    pub witness: HashSet<Role>,
    pub creator: HashSet<RoleCreator>,
    pub issuer: RoleSchemaIssuer,
}

impl RolesSchema {
    pub fn remove_member_role(&mut self, remove_members: &Vec<String>) {
        for remove in remove_members {
            self.evaluator.retain(|x| x.name != *remove);
            self.validator.retain(|x| x.name != *remove);
            self.witness.retain(|x| x.name != *remove);
            self.issuer.users.retain(|x| x.name != *remove);
            self.creator.retain(|x| x.name != *remove);
        }   
    }

    pub fn change_name_role(&mut self, chang_name_members: &Vec<(String, String)>) {
        for (old_name, new_name) in chang_name_members {
            self.evaluator = self.evaluator.iter().map(|x| {
                if x.name == *old_name {
                    Role {
                        name: new_name.clone(),
                        namespace: x.namespace.clone(),
                    }
                } else {
                    x.clone()
                }
            }).collect();

            self.validator = self.validator.iter().map(|x| {
                if x.name == *old_name {
                    Role {
                        name: new_name.clone(),
                        namespace: x.namespace.clone(),
                    }
                } else {
                    x.clone()
                }
            }).collect();

            self.witness = self.witness.iter().map(|x| {
                if x.name == *old_name {
                    Role {
                        name: new_name.clone(),
                        namespace: x.namespace.clone(),
                    }
                } else {
                    x.clone()
                }
            }).collect();

            self.creator = self.creator.iter().map(|x| {
                if x.name == *old_name {
                    RoleCreator {
                        quantity: x.quantity.clone(),
                        name: new_name.clone(),
                        namespace: x.namespace.clone(),
                    }
                } else {
                    x.clone()
                }
            }).collect();

            self.issuer.users = self.issuer.users.iter().map(|x| {
                if x.name == *old_name {
                    Role {
                        name: new_name.clone(),
                        namespace: x.namespace.clone(),
                    }
                } else {
                    x.clone()
                }
            }).collect();
        }
    }

    pub fn roles_creators(&self, name: &str, not_gov_val: Option<Vec<Namespace>>, not_gov_eval: Option<Vec<Namespace>>, not_gov_creators: Option<Vec<Role>>) -> NameCreators {
        let mut val_namespace = self.validator.iter().filter(|x| x.name == name).map(|x| x.namespace.clone()).collect::<Vec<Namespace>>();
        if let Some(mut not_gov_val) = not_gov_val {
            val_namespace.append(&mut not_gov_val);
        }

        let mut eval_namespace = self.evaluator.iter().filter(|x| x.name == name).map(|x| x.namespace.clone()).collect::<Vec<Namespace>>();
        if let Some(mut not_gov_eval) = not_gov_eval {
            eval_namespace.append(&mut not_gov_eval);
        }

        let mut creators_val: Vec<String> = vec![];
        for namespace in val_namespace.clone() {
            let mut creators = self.creator.iter().filter(|x| {
                let namespace_role = x.namespace.clone();
                namespace_role.is_ancestor_of(&namespace)
                    || namespace_role == namespace
                    || namespace_role.is_empty()
            }).map(|x| x.name.clone()).collect::<Vec<String>>();


            if let Some(not_gov_creators) = not_gov_creators.clone() {
                let mut creators = not_gov_creators.iter().filter(|x| {
                    let namespace_role = x.namespace.clone();
                    namespace_role.is_ancestor_of(&namespace)
                        || namespace_role == namespace
                        || namespace_role.is_empty()
                }).map(|x| x.name.clone()).collect::<Vec<String>>();

                creators_val.append(&mut creators);
        }


            creators_val.append(&mut creators);
        }


        let mut creators_eval: Vec<String> = vec![];
        for namespace in eval_namespace.clone() {
            let mut creators = self.creator.iter().filter(|x| {
                let namespace_role = x.namespace.clone();
                namespace_role.is_ancestor_of(&namespace)
                    || namespace_role == namespace
                    || namespace_role.is_empty()
            }).map(|x| x.name.clone()).collect::<Vec<String>>();

            if let Some(not_gov_creators) = not_gov_creators.clone() {
                let mut creators = not_gov_creators.iter().filter(|x| {
                    let namespace_role = x.namespace.clone();
                    namespace_role.is_ancestor_of(&namespace)
                        || namespace_role == namespace
                        || namespace_role.is_empty()
                }).map(|x| x.name.clone()).collect::<Vec<String>>();

                creators_eval.append(&mut creators);    
            }


            creators_eval.append(&mut creators);
        }

        let hash_val: Option<HashSet<String>> = if val_namespace.is_empty() {
            None
        } else {
            Some(HashSet::from_iter(creators_val.iter().cloned()))
        };

        let hash_eval: Option<HashSet<String>> = if eval_namespace.is_empty() {
            None
        } else {
            Some(HashSet::from_iter(creators_eval.iter().cloned()))
        };

        NameCreators {
            validation: hash_val,
            evaluation: hash_eval
        }  
    }

    pub fn roles_namespace_creators(&self, name: &str) -> (Option<Vec<Namespace>>, Option<Vec<Namespace>>, Option<Vec<Role>>) {
        let val_namespace = self.validator.iter().filter(|x| x.name == name).map(|x| x.namespace.clone()).collect::<Vec<Namespace>>();
        let eval_namespace = self.evaluator.iter().filter(|x| x.name == name).map(|x| x.namespace.clone()).collect::<Vec<Namespace>>();

        let creators = self.creator.iter().map(|x| Role {
            name: x.name.clone(),
            namespace: x.namespace.clone(),
        }).collect::<Vec<Role>>();

        let val_namespace = if val_namespace.is_empty() {
            None
        } else {
            Some(val_namespace)
        };

        let eval_namespace = if eval_namespace.is_empty() {
            None
        } else {
            Some(eval_namespace)
        };

        let creators = if creators.is_empty() {
            None
        } else {
            Some(creators)
        };

        (val_namespace, eval_namespace, creators)
    }

    pub fn hash_this_rol(
        &self,
        role: RoleTypes,
        namespace: Namespace,
        name: &str,
    ) -> bool {
        match role {
            RoleTypes::Evaluator => self.evaluator.iter().any(|x| {
                let namespace_role = x.namespace.clone();
                namespace_role.is_ancestor_of(&namespace)
                    || namespace_role == namespace
                    || namespace_role.is_empty() && x.name == name
            }),
            RoleTypes::Validator => self.validator.iter().any(|x| {
                let namespace_role = x.namespace.clone();
                namespace_role.is_ancestor_of(&namespace)
                    || namespace_role == namespace
                    || namespace_role.is_empty() && x.name == name
            }),
            RoleTypes::Witness => self.witness.iter().any(|x| {
                let namespace_role = x.namespace.clone();
                namespace_role.is_ancestor_of(&namespace)
                    || namespace_role == namespace
                    || namespace_role.is_empty() && x.name == name
            }),
            RoleTypes::Creator => self.creator.iter().any(|x| {
                let namespace_role = x.namespace.clone();
                namespace_role.is_ancestor_of(&namespace)
                    || namespace_role == namespace
                    || namespace_role.is_empty() && x.name == name
            }),
            RoleTypes::Issuer => {
                self.issuer.users.iter().any(|x| {
                    let namespace_role = x.namespace.clone();
                    namespace_role.is_ancestor_of(&namespace)
                        || namespace_role == namespace
                        || namespace_role.is_empty() && x.name == name
                }) || self.issuer.any
            }
            RoleTypes::Approver => false
        }
    }

    pub fn hash_this_rol_not_namespace(
        &self,
        role: RoleTypes,
        name: &str,
    ) -> bool {
        match role {
            RoleTypes::Evaluator => {
                self.evaluator.iter().any(|x| x.name == name)
            }
            RoleTypes::Validator => {
                self.validator.iter().any(|x| x.name == name)
            }
            RoleTypes::Witness => self.witness.iter().any(|x| x.name == name),
            RoleTypes::Creator => self.creator.iter().any(|x| x.name == name),
            RoleTypes::Issuer => {
                self.issuer.users.iter().any(|x| x.name == name)
                    || self.issuer.any
            }
            RoleTypes::Approver => false
        }
    }

    pub fn max_creations(
        &self,
        namespace: Namespace,
        name: &str,
    ) -> Option<CreatorQuantity> {
        let data = self
            .creator
            .iter()
            .find(|x| namespace == x.namespace && name == name);

        data.map(|x| x.quantity.clone())
    }

    pub fn get_signers(
        &self,
        role: RoleTypes,
        namespace: Namespace,
    ) -> (Vec<String>, bool) {
        match role {
            RoleTypes::Evaluator => (
                self.evaluator
                    .iter()
                    .filter(|x| {
                        let namespace_role = x.namespace.clone();
                        namespace_role.is_ancestor_of(&namespace)
                            || namespace_role == namespace
                            || namespace_role.is_empty()
                    })
                    .map(|x| x.name.clone())
                    .collect::<Vec<String>>(),
                false,
            ),
            RoleTypes::Validator => (
                self.validator
                    .iter()
                    .filter(|x| {
                        let namespace_role = x.namespace.clone();
                        namespace_role.is_ancestor_of(&namespace)
                            || namespace_role == namespace
                            || namespace_role.is_empty()
                    })
                    .map(|x| x.name.clone())
                    .collect::<Vec<String>>(),
                false,
            ),
            RoleTypes::Witness => (
                self.witness
                    .iter()
                    .filter(|x| {
                        let namespace_role = x.namespace.clone();
                        namespace_role.is_ancestor_of(&namespace)
                            || namespace_role == namespace
                            || namespace_role.is_empty()
                    })
                    .map(|x| x.name.clone())
                    .collect::<Vec<String>>(),
                false,
            ),
            RoleTypes::Creator => (
                self.creator
                    .iter()
                    .filter(|x| {
                        let namespace_role = x.namespace.clone();
                        namespace_role.is_ancestor_of(&namespace)
                            || namespace_role == namespace
                            || namespace_role.is_empty()
                    })
                    .map(|x| x.name.clone())
                    .collect::<Vec<String>>(),
                false,
            ),
            RoleTypes::Issuer => (
                self.issuer
                    .users
                    .iter()
                    .filter(|x| {
                        let namespace_role = x.namespace.clone();
                        namespace_role.is_ancestor_of(&namespace)
                            || namespace_role == namespace
                            || namespace_role.is_empty()
                    })
                    .map(|x| x.name.clone())
                    .collect::<Vec<String>>(),
                self.issuer.any,
            ),
            RoleTypes::Approver => (vec![], false)
        }
    }
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RoleTypes {
    Approver,
    Evaluator,
    Validator,
    Witness,
    Creator,
    Issuer,
}

impl From<ProtocolTypes> for RoleTypes {
    fn from(value: ProtocolTypes) -> Self {
        match value {
            ProtocolTypes::Aprovation => RoleTypes::Approver,
            ProtocolTypes::Evaluation => RoleTypes::Evaluator,
            ProtocolTypes::Validation => RoleTypes::Validator,
        }
    }
}

/// Governance role.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash, Eq, PartialOrd, Ord)]
pub struct Role {
    pub name: String,
    pub namespace: Namespace,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialOrd, Ord)]
pub struct RoleCreator {
    pub name: String,
    pub namespace: Namespace,
    pub quantity: CreatorQuantity,
}

impl Hash for RoleCreator {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.namespace.hash(state);
    }
}

impl PartialEq for RoleCreator{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.namespace == other.namespace
    }
}

impl Eq for RoleCreator {}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RoleGovIssuer {
    pub users: HashSet<MemberName>,
    pub any: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct RoleSchemaIssuer {
    pub users: HashSet<Role>,
    pub any: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum CreatorQuantity {
    Quantity(u32),
    Infinity,
}

impl fmt::Display for CreatorQuantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreatorQuantity::Quantity(quantity) => write!(f, "{}", quantity),
            CreatorQuantity::Infinity => write!(f, "Infinity"),
        }
    }
}

/// Governance member.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct Member {
    pub id: KeyIdentifier,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ProtocolTypes {
    Aprovation,
    Evaluation,
    Validation,
}

impl fmt::Display for ProtocolTypes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolTypes::Aprovation => write!(f, "Aprovation"),
            ProtocolTypes::Evaluation => write!(f, "Evaluation"),
            ProtocolTypes::Validation => write!(f, "Validation"),
        }
    }
}

/// Governance quorum.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub enum Quorum {
    #[default]
    Majority,
    Fixed(u32),
    Percentage(u8),
}

impl Quorum {
    pub fn check_values(&self) -> Result<(), String> {
        match self {
            Quorum::Percentage(percentage) => if *percentage == 0_u8 || *percentage > 100_u8 {
                return Err("the percentage must be between 1 and 100".to_owned());
            },
            _ => {}
        }

        Ok(())
    }

    pub fn check_quorum(&self, total_members: u32, signers: u32) -> bool {
        match self {
            Quorum::Fixed(fixed) => {
                let min = std::cmp::min(fixed, &total_members);
                signers >= *min
            }
            Quorum::Majority => signers > total_members / 2,
            Quorum::Percentage(percentage) => {
                signers >= (total_members * (percentage / 100) as u32)
            }
        }
    }
}

/// Governance policy.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PolicyGov {
    /// Approve quorum
    pub approve: Quorum,
    /// Evaluate quorum
    pub evaluate: Quorum,
    /// Validate quorum
    pub validate: Quorum,
}

impl PolicyGov {
    pub fn get_quorum(&self, role: ProtocolTypes) -> Option<Quorum> {
        match role {
            ProtocolTypes::Aprovation => Some(self.approve.clone()),
            ProtocolTypes::Evaluation => Some(self.evaluate.clone()),
            ProtocolTypes::Validation => Some(self.validate.clone()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Hash, PartialEq, Eq, Default)]
pub struct PolicySchema {
    /// Evaluate quorum
    pub evaluate: Quorum,
    /// Validate quorum
    pub validate: Quorum,
}

impl PolicySchema {
    pub fn get_quorum(&self, role: ProtocolTypes) -> Option<Quorum> {
        match role {
            ProtocolTypes::Aprovation => None,
            ProtocolTypes::Evaluation => Some(self.evaluate.clone()),
            ProtocolTypes::Validation => Some(self.validate.clone()),
        }
    }
}
