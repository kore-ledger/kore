// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance schema.
//!

use serde_json::{Value, json};

pub fn schema() -> Value {
    json!({
      "$defs": {
        "quorum": {
          "oneOf": [
            {
              "type": "string",
              "enum": ["MAJORITY"]
            },
            {
              "type": "object",
              "properties": {
                "FIXED": {
                  "type": "number",
                  "minimum": 1,
                  "multipleOf": 1
                }
              },
              "required": ["FIXED"],
              "additionalProperties": false
            },
            {
              "type": "object",
              "properties": {
                "PERCENTAGE": {
                  "type": "number",
                  "minimum": 0,
                  "maximum": 1
                }
              },
              "required": ["PERCENTAGE"],
              "additionalProperties": false
            },
          ]
        }
      },
      "type": "object",
      "additionalProperties": false,
      "required": [
        "version",
        "members",
        "schemas",
        "policies",
        "roles"
      ],
      "properties": {
        "version": {
          "type": "number",
          "minimum": 0
        },
        "members": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "name": {
                "type": "string"
              },
              "id": {
                "type": "string",
                "format": "keyidentifier"
              },
              "description": {
                "type": "string"
              }
            },
            "required": [
              "id",
              "name"
            ],
            "additionalProperties": false
          }
        },
        "roles": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "who": {
                "oneOf": [
                {
                  "type": "object",
                  "properties": {
                    "ID": {
                      "type": "string"
                    }
                  },
                  "required": ["ID"],
                  "additionalProperties": false
                },
                {
                  "type": "object",
                  "properties": {
                    "NAME": {
                      "type": "string"
                    }
                  },
                  "required": ["NAME"],
                  "additionalProperties": false
                },
                {
                  "const": "MEMBERS"
                },
                {
                  "const": "ALL"
                },
                {
                  "const": "NOT_MEMBERS"
                }
              ]
            },
            "namespace": {
              "type": "string"
            },
              "role": {
                "oneOf": [
                  {
                    "type": "object",
                    "properties": {
                      "CREATOR": {
                        "oneOf": [
                          {
                            "type": "object",
                            "properties": {
                              "QUANTITY": {
                                "type": "integer",
                                "minimum": 1
                              }
                            },
                            "required": ["QUANTITY"]
                          },
                          {
                            "type": "string",
                            "enum": ["INFINITY"]
                          }
                        ]
                      }
                    },
                    "required": ["CREATOR"]
                  },
                  {
                    "type": "string",
                    "enum": ["APPROVER", "EVALUATOR", "VALIDATOR", "WITNESS", "ISSUER"]
                  }
                ]
              },
            "schema": {
              "oneOf": [
                {
                  "type": "object",
                  "properties": {
                    "ID": {
                      "type": "string"
                    }
                  },
                  "required": ["ID"],
                  "additionalProperties": false
                },
                {
                  "const": "ALL"
                },
                {
                  "const": "NOT_GOVERNANCE"
                }
                ]
              }
            },
            "required": ["who", "role", "schema", "namespace"],
            "additionalProperties": false
          }
        },
        "schemas": {
          "type": "array",
          "minItems": 0,
          "items": {
            "type": "object",
            "properties": {
              "id": {
                "type": "string"
              },
              "initial_value": {},
              "contract": {
                "type": "object",
                "properties": {
                  "raw": {
                    "type": "string"
                  },
                },
                "additionalProperties": false,
                "required": ["raw"]
              },
            },
            "required": [
              "id",
              "initial_value",
              "contract"
            ],
            "additionalProperties": false
          }
        },
        "policies": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": [
              "id", "approve", "evaluate", "validate"
            ],
            "properties": {
              "id": {
                "type": "string"
              },
              "approve": {
                "type": "object",
                "additionalProperties": false,
                "required": ["quorum"],
                "properties": {
                  "quorum": {
                    "$ref": "#/$defs/quorum"
                  }
                }
              },
              "evaluate": {
                "type": "object",
                "additionalProperties": false,
                "required": ["quorum"],
                "properties": {
                  "quorum": {
                    "$ref": "#/$defs/quorum"
                  }
                }
              },
              "validate": {
                "type": "object",
                "additionalProperties": false,
                "required": ["quorum"],
                "properties": {
                  "quorum": {
                    "$ref": "#/$defs/quorum"
                  }
                }
              }
            }
          }
        }
      }
    })
}
