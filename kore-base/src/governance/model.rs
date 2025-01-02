// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance model.
//!

use serde::{de::Visitor, ser::SerializeMap, Deserialize, Serialize};

use std::{
    fmt::{self},
    hash::Hasher,
};

/// Governance quorum.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(non_snake_case)]
#[allow(clippy::upper_case_acronyms)]
pub enum Quorum {
    #[default]
    MAJORITY,
    FIXED(u32),
    PERCENTAGE(f64), // BFT { BFT: f64 },
}

impl Quorum {
    pub fn check_quorum(&self, total_members: u32, signers: u32) -> bool {
        match self {
            Quorum::FIXED(fixed) => {
                let min = std::cmp::min(fixed, &total_members);
                signers >= *min
            }
            Quorum::MAJORITY => signers > total_members / 2,
            Quorum::PERCENTAGE(percentage) => {
                signers >= ((total_members as f64 * percentage).ceil() as u32)
            }
        }
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
    NOT_MEMBERS,
}

impl fmt::Display for Who {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Who::ID { ID } => write!(f, "ID: {}", ID),
            Who::NAME { NAME } => write!(f, "NAME: {}", NAME),
            Who::MEMBERS => write!(f, "MEMBERS"),
            Who::NOT_MEMBERS => write!(f, "NOT_MEMBERS"),
        }
    }
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
                    "NOT_MEMBERS" => Ok(Who::NOT_MEMBERS),
                    other => Err(serde::de::Error::unknown_variant(
                        other,
                        &["MEMBERS", "NOT_MEMBERS"],
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
                    "NOT_MEMBERS" => Ok(Who::NOT_MEMBERS),
                    other => Err(serde::de::Error::unknown_variant(
                        other,
                        &["MEMBERS", "NOT_MEMBERS"],
                    )),
                }
            }
        }
        deserializer.deserialize_any(WhoVisitor {})
    }
}

/// Governance schema enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub initial_value: serde_json::Value,
    pub contract: Contract,
}

impl PartialEq for Schema {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// Governance role.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Role {
    pub who: Who,
    pub namespace: String,
    pub role: Roles,
    pub schema: SchemaEnum,
}

#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize, Clone, Eq)]
pub enum Roles {
    APPROVER,
    EVALUATOR,
    VALIDATOR,
    WITNESS,
    CREATOR(CreatorQuantity),
    ISSUER,
}

// Implementación personalizada de PartialEq
impl PartialEq for Roles {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Roles::APPROVER, Roles::APPROVER)
                | (Roles::EVALUATOR, Roles::EVALUATOR)
                | (Roles::VALIDATOR, Roles::VALIDATOR)
                | (Roles::WITNESS, Roles::WITNESS)
                | (Roles::ISSUER, Roles::ISSUER)
                | (Roles::CREATOR { .. }, Roles::CREATOR { .. })
        )
    }
}

impl std::hash::Hash for Roles {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Roles::APPROVER => state.write_u8(0),
            Roles::EVALUATOR => state.write_u8(1),
            Roles::VALIDATOR => state.write_u8(2),
            Roles::WITNESS => state.write_u8(3),
            Roles::CREATOR { .. } => state.write_u8(4),
            Roles::ISSUER => state.write_u8(5),
        }
    }
}

impl Roles {
    pub fn to_str(&self) -> &str {
        match self {
            Roles::APPROVER => "approver",
            Roles::EVALUATOR => "evaluator",
            Roles::VALIDATOR => "validator",
            Roles::WITNESS => "witness",
            Roles::CREATOR(_) => "creator",
            Roles::ISSUER => "issuer",
        }
    }
}

impl fmt::Display for Roles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Roles::APPROVER => write!(f, "Approver"),
            Roles::EVALUATOR => write!(f, "Evaluator"),
            Roles::VALIDATOR => write!(f, "Validator"),
            Roles::WITNESS => write!(f, "Witness"),
            Roles::CREATOR(quantity) => {
                write!(f, "Creator who can create {} subjects", quantity)
            }
            Roles::ISSUER => write!(f, "Issuer"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum CreatorQuantity {
    QUANTITY(u32),
    INFINITY,
}

impl fmt::Display for CreatorQuantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreatorQuantity::QUANTITY(quantity) => write!(f, "{}", quantity),
            CreatorQuantity::INFINITY => write!(f, "Infinity"),
        }
    }
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

/// Request stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RequestStage {
    Evaluate,
    Approve,
    Validate,
    Witness,
}

impl RequestStage {
    pub fn to_str(&self) -> &str {
        match self {
            RequestStage::Approve => "approve",
            RequestStage::Evaluate => "evaluate",
            RequestStage::Validate => "validate",
            RequestStage::Witness => "witness",
        }
    }

    pub fn to_role(&self) -> &str {
        match self {
            RequestStage::Approve => Roles::APPROVER.to_str(),
            RequestStage::Evaluate => Roles::EVALUATOR.to_str(),
            RequestStage::Validate => Roles::VALIDATOR.to_str(),
            RequestStage::Witness => Roles::WITNESS.to_str(),
        }
    }
}
