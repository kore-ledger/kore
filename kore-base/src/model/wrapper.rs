// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Wrapper for Json Value.
//!

use super::HashId;

use crate::Error;

use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use tracing::error;

use core::str;
use std::{
    io::{Read, Write},
    str::FromStr,
};

const TARGET_WRAPPER: &str = "Kore-Model-Wrapper";

/// Wrapper of serde_json::Value implementing serialization and deserialization with Borsh.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ValueWrapper(pub Value);

impl ValueWrapper {
    pub fn as_str(&self) -> Option<&str> {
        self.0.as_str()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }
}

impl Default for ValueWrapper {
    fn default() -> Self {
        ValueWrapper(Value::Null)
    }
}

impl HashId for ValueWrapper {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_WRAPPER, "HashId for SubjectID fails: {}", e);
                Error::HashID(format!("HashId for SubjectID fails: {}", e))
            },
        )
    }
}

impl Serialize for ValueWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let json_string = serde_json::to_string(&self.0)
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&json_string)
    }
}

impl<'de> Deserialize<'de> for ValueWrapper {
    fn deserialize<D>(deserializer: D) -> Result<ValueWrapper, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s =
            <std::string::String as Deserialize>::deserialize(deserializer)?;
        let value = Value::from_str(&s).map_err(serde::de::Error::custom)?;
        Ok(ValueWrapper(value))
    }
}

impl BorshSerialize for ValueWrapper {
    #[inline]
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match &self.0 {
            Value::Bool(data) => {
                BorshSerialize::serialize(&0u8, writer)?;
                BorshSerialize::serialize(&data, writer)
            }
            Value::Number(data) => {
                BorshSerialize::serialize(&1u8, writer)?;
                'data: {
                    if data.is_f64() {
                        let Some(data) = data.as_f64() else {
                            break 'data;
                        };
                        BorshSerialize::serialize(&0u8, writer)?;
                        return BorshSerialize::serialize(&data, writer);
                    } else if data.is_i64() {
                        let Some(data) = data.as_i64() else {
                            break 'data;
                        };
                        BorshSerialize::serialize(&1u8, writer)?;
                        return BorshSerialize::serialize(&data, writer);
                    } else if data.is_u64() {
                        let Some(data) = data.as_u64() else {
                            break 'data;
                        };
                        BorshSerialize::serialize(&2u8, writer)?;
                        return BorshSerialize::serialize(&data, writer);
                    }
                }
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid number type",
                ))
            }
            Value::String(data) => {
                BorshSerialize::serialize(&2u8, writer)?;
                BorshSerialize::serialize(&data, writer)
            }
            Value::Array(data) => {
                BorshSerialize::serialize(&3u8, writer)?;
                BorshSerialize::serialize(&(data.len() as u32), writer)?;
                for element in data {
                    let element = ValueWrapper(element.to_owned());
                    BorshSerialize::serialize(&element, writer)?;
                }
                Ok(())
            }
            Value::Object(data) => {
                BorshSerialize::serialize(&4u8, writer)?;
                BorshSerialize::serialize(&(data.len() as u32), writer)?;
                for (key, value) in data {
                    BorshSerialize::serialize(&key, writer)?;
                    let value = ValueWrapper(value.to_owned());
                    BorshSerialize::serialize(&value, writer)?;
                }
                Ok(())
            }
            Value::Null => BorshSerialize::serialize(&5u8, writer),
        }
    }
}

impl BorshDeserialize for ValueWrapper {
    #[inline]
    fn deserialize_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let order: u8 = BorshDeserialize::deserialize_reader(reader)?;
        match order {
            0 => {
                let data: bool = BorshDeserialize::deserialize_reader(reader)?;
                Ok(ValueWrapper(Value::Bool(data)))
            }
            1 => {
                let internal_order: u8 =
                    BorshDeserialize::deserialize_reader(reader)?;
                match internal_order {
                    0 => {
                        let data: f64 =
                            BorshDeserialize::deserialize_reader(reader)?;
                        let Some(data_f64) = Number::from_f64(data) else {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                format!("Invalid f64 Number: {}", data),
                            ));
                        };
                        Ok(ValueWrapper(Value::Number(data_f64)))
                    }
                    1 => {
                        let data: i64 =
                            BorshDeserialize::deserialize_reader(reader)?;
                        Ok(ValueWrapper(Value::Number(Number::from(data))))
                    }
                    2 => {
                        let data: u64 =
                            BorshDeserialize::deserialize_reader(reader)?;
                        Ok(ValueWrapper(Value::Number(Number::from(data))))
                    }
                    _ => Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "Invalid Number representation: {}",
                            internal_order
                        ),
                    )),
                }
            }
            2 => {
                let data: String =
                    BorshDeserialize::deserialize_reader(reader)?;
                Ok(ValueWrapper(Value::String(data)))
            }
            3 => {
                let len = u32::deserialize_reader(reader)?;
                if len == 0 {
                    Ok(ValueWrapper(Value::Array(Vec::new())))
                } else {
                    let mut result = Vec::with_capacity(len as usize);
                    for _ in 0..len {
                        result
                            .push(ValueWrapper::deserialize_reader(reader)?.0);
                    }
                    Ok(ValueWrapper(Value::Array(result)))
                }
            }
            4 => {
                let len = u32::deserialize_reader(reader)?;
                let mut result = Map::new();
                for _ in 0..len {
                    let key = String::deserialize_reader(reader)?;
                    let value = ValueWrapper::deserialize_reader(reader)?;
                    result.insert(key, value.0);
                }
                Ok(ValueWrapper(Value::Object(result)))
            }
            5 => Ok(ValueWrapper(Value::Null)),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid Value representation: {}", order),
            )),
        }
    }
}

#[cfg(test)]
mod tests {

    use identity::identifier::KeyIdentifier;

    use crate::governance::Governance;

    use super::*;

    #[test]
    fn test_value_wrapper() {
        let value = Value::String("test".to_string());
        let wrapper = ValueWrapper(value.clone());
        let mut buffer: Vec<u8> = Vec::new();
        borsh::BorshSerialize::serialize(&wrapper, &mut buffer).unwrap();
        let deserialized: ValueWrapper =
            borsh::BorshDeserialize::deserialize(&mut buffer.as_slice())
                .unwrap();
        assert_eq!(deserialized.0, value);
    }

    #[test]
    fn test_serialize_deserialize() {
        let value = Governance::new(KeyIdentifier::default())
            .to_value_wrapper()
            .unwrap();
        let bytes =
            bincode::serde::encode_to_vec(value, bincode::config::standard())
                .unwrap();
        let wrapper = bincode::serde::decode_from_slice::<ValueWrapper, _>(
            &bytes,
            bincode::config::standard(),
        );
        wrapper.unwrap();
    }
}
