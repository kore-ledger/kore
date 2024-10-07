// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance init state.
//!

use crate::model::ValueWrapper;

// TODO: el rol de ISSUER es el único que no tiene que ser testigo de la governanza

pub fn init_state(owner_key: &str) -> ValueWrapper {
    ValueWrapper(serde_json::json!({
        "members": [
          {
            "id": owner_key,
            "name": "Owner"
          }
        ],
        "roles": [
          {
            "namespace": "",
            "role": "WITNESS",
            "schema": {
                "ID": "governance"
            },
            "who": "MEMBERS"
          },
          {
            "namespace": "",
            "role": "EVALUATOR",
            "schema": "ALL",
            "who": {
              "NAME": "Owner"
            }
          },
          {
            "namespace": "",
            "role": "APPROVER",
            "schema": "ALL",
            "who": {
              "NAME": "Owner"
            }
          },
          {
            "namespace": "",
            "role": "VALIDATOR",
            "schema": "ALL",
            "who": {
              "NAME": "Owner"
            }
          },
          {
            "namespace": "",
            "role": "WITNESS",
            "schema": "ALL",
            "who": {
              "NAME": "Owner"
            }
          },
        ],
        "schemas": [],
        "policies": [
          {
            "id": "governance",
            "approve": {
              "quorum": "MAJORITY"
            },
            "evaluate": {
              "quorum": "MAJORITY"
            },
            "validate": {
              "quorum": "MAJORITY"
            }
          }
        ]
    }))
}

#[cfg(test)]
mod test {
    use crate::governance::{json_schema::JsonSchema, schema};

    use super::init_state;

    #[test]
    fn gov_schema_init_state() {
        let init_state =
            init_state("EUrVnqpwo9EKBvMru4wWLMpJgOTKM5gZnxApRmjrRbbE");
        let schema = JsonSchema::compile(&schema()).unwrap();
        assert_eq!(schema.fast_validate(&init_state.0), true);
    }
}
