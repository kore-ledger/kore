// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance model.
//!

use identity::identifier::DigestIdentifier;

use serde::{de::Visitor, ser::SerializeMap, Deserialize, Serialize};

use std::collections::HashSet;

/// Governance quorum.
#[derive(Debug, Clone)]
#[allow(non_snake_case)]
#[allow(clippy::upper_case_acronyms)]
pub enum Quorum {
    MAJORITY,
    FIXED { fixed: u32 },
    PERCENTAGE { percentage: f64 },
    // BFT { BFT: f64 },
}

impl Quorum {
    pub fn check_quorum(&self, total_members: u32, signers: u32) -> bool {
        match self {
            Quorum::FIXED { fixed } => {
                let min = std::cmp::min(fixed, &total_members);
                signers >= *min
            }
            Quorum::MAJORITY => signers >= total_members / 2 + 1,
            Quorum::PERCENTAGE { percentage } => {
                signers >= ((total_members as f64 * percentage).ceil() as u32)
            }
        }
    }
}

impl Serialize for Quorum {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Quorum::MAJORITY => serializer.serialize_str("MAJORITY"),
            Quorum::FIXED { fixed } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("FIXED", fixed)?;
                map.end()
            }
            Quorum::PERCENTAGE { percentage } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("PERCENTAGE", percentage)?;
                map.end()
            } // Quorum::BFT { BFT } => {
              //     let mut map = serializer.serialize_map(Some(1))?;
              //     map.serialize_entry("BFT", BFT)?;
              //     map.end()
              // }
        }
    }
}

impl<'de> Deserialize<'de> for Quorum {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct QuorumEnumVisitor;
        impl<'de> Visitor<'de> for QuorumEnumVisitor {
            type Value = Quorum;
            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str("Quorum")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                // Solo deberían tener una entrada
                let Some(key) = map.next_key::<String>()? else {
                    return Err(serde::de::Error::missing_field(
                        "FIXED or PERCENTAGE",
                    ));
                };
                let result = match key.as_str() {
                    "FIXED" => {
                        let fixed: u32 = map.next_value()?;
                        Quorum::FIXED { fixed }
                    }
                    // "BFT" => {
                    //     let bft: f64 = map.next_value()?;
                    //     Quorum::BFT { BFT: bft }
                    // }
                    "PERCENTAGE" => {
                        let percentage: f64 = map.next_value()?;
                        Quorum::PERCENTAGE { percentage }
                    }
                    _ => {
                        return Err(serde::de::Error::unknown_field(
                            &key,
                            &["FIXED", "PERCENTAGE"],
                        ))
                    }
                };
                let None = map.next_key::<String>()? else {
                    return Err(serde::de::Error::custom(
                        "Input data is not valid. The data contains unkown entries",
                    ));
                };
                Ok(result)
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&v)
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    "MAJORITY" => Ok(Self::Value::MAJORITY),
                    other => Err(serde::de::Error::unknown_variant(
                        other,
                        &["MAJORITY"],
                    )),
                }
            }
            fn visit_borrowed_str<E>(
                self,
                v: &'de str,
            ) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    "MAJORITY" => Ok(Self::Value::MAJORITY),
                    other => Err(serde::de::Error::unknown_variant(
                        other,
                        &["MAJORITY"],
                    )),
                }
            }
        }
        deserializer.deserialize_any(QuorumEnumVisitor {})
    }
}

/// Governance who.
#[derive(Debug, Clone)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub enum Who {
    ID { ID: String },
    NAME { NAME: String },
    MEMBERS,
    ALL,
    NOT_MEMBERS,
}

impl Serialize for Who {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Who::ID { ID } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("ID", ID)?;
                map.end()
            }
            Who::NAME { NAME } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("NAME", NAME)?;
                map.end()
            }
            Who::MEMBERS => serializer.serialize_str("MEMBERS"),
            Who::ALL => serializer.serialize_str("ALL"),
            Who::NOT_MEMBERS => serializer.serialize_str("NOT_MEMBERS"),
        }
    }
}

impl<'de> Deserialize<'de> for Who {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct WhoVisitor;
        impl<'de> Visitor<'de> for WhoVisitor {
            type Value = Who;
            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str("Who")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                // Solo deberían tener una entrada
                let Some(key) = map.next_key::<String>()? else {
                    return Err(serde::de::Error::missing_field("ID or NAME"));
                };
                let result = match key.as_str() {
                    "ID" => {
                        let id: String = map.next_value()?;
                        Who::ID { ID: id }
                    }
                    "NAME" => {
                        let name: String = map.next_value()?;
                        Who::NAME { NAME: name }
                    }
                    _ => {
                        return Err(serde::de::Error::unknown_field(
                            &key,
                            &["ID", "NAME"],
                        ))
                    }
                };
                let None = map.next_key::<String>()? else {
                    return Err(serde::de::Error::custom(
                        "Input data is not valid. The data contains unkown entries",
                    ));
                };
                Ok(result)
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&v)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    "MEMBERS" => Ok(Who::MEMBERS),
                    "ALL" => Ok(Who::ALL),
                    "NOT_MEMBERS" => Ok(Who::NOT_MEMBERS),
                    other => Err(serde::de::Error::unknown_variant(
                        other,
                        &["MEMBERS", "ALL", "NOT_MEMBERS"],
                    )),
                }
            }

            fn visit_borrowed_str<E>(
                self,
                v: &'de str,
            ) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    "MEMBERS" => Ok(Who::MEMBERS),
                    "ALL" => Ok(Who::ALL),
                    "NOT_MEMBERS" => Ok(Who::NOT_MEMBERS),
                    other => Err(serde::de::Error::unknown_variant(
                        other,
                        &["MEMBERS", "ALL", "NOT_MEMBERS"],
                    )),
                }
            }
        }
        deserializer.deserialize_any(WhoVisitor {})
    }
}

/// Governance schema enumeration.
#[derive(Debug, Clone)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub enum SchemaEnum {
    ID { ID: String },
    NOT_GOVERNANCE,
    ALL,
}

impl Serialize for SchemaEnum {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            SchemaEnum::ID { ID } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("ID", ID)?;
                map.end()
            }
            SchemaEnum::NOT_GOVERNANCE => {
                serializer.serialize_str("NOT_GOVERNANCE")
            }
            SchemaEnum::ALL => serializer.serialize_str("ALL"),
        }
    }
}

impl<'de> Deserialize<'de> for SchemaEnum {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SchemaEnumVisitor;
        impl<'de> Visitor<'de> for SchemaEnumVisitor {
            type Value = SchemaEnum;
            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str("Schema")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                // Solo deberían tener una entrada
                let Some(key) = map.next_key::<String>()? else {
                    return Err(serde::de::Error::missing_field("ID"));
                };
                let result = match key.as_str() {
                    "ID" => {
                        let id: String = map.next_value()?;
                        SchemaEnum::ID { ID: id }
                    }
                    _ => {
                        return Err(serde::de::Error::unknown_field(
                            &key,
                            &["ID"],
                        ))
                    }
                };
                let None = map.next_key::<String>()? else {
                    return Err(serde::de::Error::custom(
                        "Input data is not valid. The data contains unkown entries",
                    ));
                };
                Ok(result)
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&v)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    "ALL" => Ok(Self::Value::ALL),
                    "NOT_GOVERNANCE" => Ok(Self::Value::NOT_GOVERNANCE),
                    other => Err(serde::de::Error::unknown_variant(
                        other,
                        &["ALL", "NOT_GOVERNANCE"],
                    )),
                }
            }

            fn visit_borrowed_str<E>(
                self,
                v: &'de str,
            ) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    "ALL" => Ok(Self::Value::ALL),
                    "NOT_GOVERNANCE" => Ok(Self::Value::NOT_GOVERNANCE),
                    other => Err(serde::de::Error::unknown_variant(
                        other,
                        &["ALL", "NOT_GOVERNANCE"],
                    )),
                }
            }
        }
        deserializer.deserialize_any(SchemaEnumVisitor {})
    }
}

/// Governance schema.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Schema {
    pub id: String,
    pub schema: serde_json::Value,
    pub initial_value: serde_json::Value,
    pub contract: Contract,
}

/// Governance role.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Role {
    pub who: Who,
    pub namespace: String,
    pub role: String,
    pub schema: SchemaEnum,
}

/// Governance contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    pub raw: String,
}

/// Governance member.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct Member {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Governance validation (from quorum).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Validation {
    pub quorum: Quorum,
}

/// Governance policy.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Policy {
    /// Schema id.
    pub id: String,
    /// Approve quorum
    pub approve: Validation,
    /// Evaluate quorum
    pub evaluate: Validation,
    /// Validate quorum
    pub validate: Validation,
}

/// Governance model.
/// This struct represents the governance model.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GovernanceModel {
    /// The version of the governance model.
    pub version: u64,
    /// The set of subjects identifiers directed by this governance.
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

/// Request stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum RequestStage {
    Approve,
    Evaluate,
    Validate,
    Witness,
    #[default]
    Create,
    Invoke,
}

impl RequestStage {
    pub fn to_str(&self) -> &str {
        match self {
            RequestStage::Approve => "approve",
            RequestStage::Evaluate => "evaluate",
            RequestStage::Validate => "validate",
            RequestStage::Witness => "witness",
            RequestStage::Create => "create",
            RequestStage::Invoke => "issue",
        }
    }

    pub fn to_role(&self) -> &str {
        match self {
            RequestStage::Approve => "APPROVER",
            RequestStage::Evaluate => "EVALUATOR",
            RequestStage::Validate => "VALIDATOR",
            RequestStage::Witness => "WITNESS",
            RequestStage::Create => "CREATOR",
            RequestStage::Invoke => "ISSUER",
        }
    }
}
