use std::str::FromStr;

use identity::identifier::KeyIdentifier;
use jsonschema::{Draft, Validator};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::error;

use crate::Error;

pub struct JsonSchema {
    json_schema: Validator,
}

// TODO revisar esto.
impl JsonSchema {
    pub fn compile(schema: &Value) -> Result<Self, Error> {
        match jsonschema::options()
            .with_draft(Draft::Draft202012)
            .with_format("keyidentifier", validate_gov_keyidentifiers)
            .build(schema)
        {
            Ok(json_schema) => Ok(JsonSchema { json_schema }),
            Err(e) => Err(Error::JSONSChema(format!("{}", e))),
        }
    }

    pub fn fast_validate(&self, value: &Value) -> bool {
        self.json_schema.is_valid(value)
    }

    pub fn validate(&self, value: &Value) -> bool {
        match self.json_schema.validate(value) {
            Ok(_) => true,
            Err(e) => {
                for error in e {
                    println!("schema validation error: {:?}", error);
                }
                false
            }
        }
    }
}

fn validate_gov_keyidentifiers(key: &str) -> bool {
    KeyIdentifier::from_str(key).is_ok()
}
