use std::str::FromStr;

use identity::identifier::KeyIdentifier;
use jsonschema::JSONSchema;
use serde_json::Value;
use tracing::error;

use crate::Error;

pub struct JsonSchema {
    json_schema: JSONSchema,
}

// TODO revisar esto.
impl JsonSchema {
    pub fn compile(schema: &Value) -> Result<Self, Error> {
        match JSONSchema::options()
            .with_format("keyidentifier", validate_gov_keyidentifiers)
            .compile(schema)
        {
            Ok(json_schema) => Ok(JsonSchema { json_schema }),
            Err(e) => Err(Error::JSONSChema(format!("{}", e))),
        }
    }

    pub fn validate(&self, value: &Value) -> bool {
        match self.json_schema.validate(value) {
            Ok(_) => true,
            Err(e) => {
                for error in e {
                    error!("schema validation error: {:?}", error);
                }
                false
            }
        }
    }
}

fn validate_gov_keyidentifiers(key: &str) -> bool {
    KeyIdentifier::from_str(key).is_ok()
}
