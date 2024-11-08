use std::str::FromStr;

use identity::identifier::KeyIdentifier;
use jsonschema::{Draft, Validator};
use serde_json::Value;

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

    pub fn validate(&self, value: &Value) -> Result<(), Error> {
        self.json_schema.validate(value).map_err(|e| Error::JSONSChema(format!("{}", e)))
    }
}

fn validate_gov_keyidentifiers(key: &str) -> bool {
    KeyIdentifier::from_str(key).is_ok()
}

/*

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::JsonSchema;

    #[test]
    fn a() {
        let schema = json!(
 {
                                    "additionalProperties": false,
                                    "description": "Representation of a production system",
                                    "properties": {
                                        "name": {
                                            "description": "Name of the production system",
                                            "type": "string"
                                        },
                                        "version": {
                                            "description": "Version of the production system",
                                            "type": "integer"
                                        },
                                        "unit_process": {
                                            "description": "List of unit processes in the production system",
                                            "type": "array",
                                            "items": {
                                                "additionalProperties": false,
                                                "description": "A single unit process within the production system",
                                                "properties": {
                                                    "name": {
                                                        "description": "Name of the unit process",
                                                        "type": "string"
                                                    },
                                                    "main_output": {
                                                        "$ref": "#/definitions/Element"
                                                    },
                                                    "inputs": {
                                                        "description": "List of input elements for the unit process",
                                                        "type": "array",
                                                        "items": {
                                                            "$ref": "#/definitions/Element"
                                                        }
                                                    },
                                                    "other_outputs": {
                                                        "description": "List of other outputs from the unit process",
                                                        "type": "array",
                                                        "items": {
                                                            "$ref": "#/definitions/Element"
                                                        }
                                                    },
                                                    "scope": {
                                                        "$ref": "#/definitions/Scopes"
                                                    }
                                                },
                                                "type": "object"
                                            }
                                        }
                                    },
                                    "required": [
                                        "name",
                                        "version",
                                        "unit_process"
                                    ],
                                    "type": "object",
                                    "definitions": {
                                        "Element": {
                                            "additionalProperties": false,
                                            "description": "An element associated with a unit process",
                                            "properties": {
                                                "name": {
                                                    "description": "Name of the element (e.g., oil flow rate)",
                                                    "type": "string"
                                                },
                                                "element_type": {
                                                    "description": "Type of the element (e.g., WaterFlow)",
                                                    "type": "string"
                                                },
                                                "category": {
                                                    "description": "Category of the element (e.g., Water)",
                                                    "type": "string"
                                                },
                                                "element_name": {
                                                    "description": "Specific name of the element (e.g., volumetric flow rate)",
                                                    "type": "string"
                                                },
                                                "unit": {
                                                    "description": "Unit of measurement for the element (e.g., m^3/h)",
                                                    "type": "string"
                                                },
                                                "measure": {
                                                    "description": "Measurement value of the element",
                                                    "type": "integer"
                                                },
                                                "scope": {
                                                    "$ref": "#/definitions/Scopes"
                                                }
                                            },
                                            "required": [
                                                "name",
                                                "element_type",
                                                "category",
                                                "element_name",
                                                "unit"
                                            ],
                                            "type": "object"
                                        },
                                        "Scopes": {
                                            "additionalProperties": false,
                                            "description": "Temporal scope associated with the element or unit process",
                                            "properties": {
                                                "temporal": {
                                                    "description": "Temporal value in seconds",
                                                    "type": "integer"
                                                }
                                            },
                                            "required": [
                                                "temporal"
                                            ],
                                            "type": "object"
                                        }
                                    }
                                }
        );
        let a = JsonSchema::compile(&schema).unwrap();
        let b =  json!(
            {
                "name": "",
                "version": 0
            }
        );
        a.validate(&b).unwrap()
    }
}
*/
