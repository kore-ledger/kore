// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance init state.
//!

use crate::model::ValueWrapper;

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
            "schema": {
              "ID": "governance"
            },
            "who": {
              "NAME": "Owner"
            }
          },
          {
            "namespace": "",
            "role": "APPROVER",
            "schema": {
              "ID": "governance"
            },
            "who": {
              "NAME": "Owner"
            }
          },
          {
            "namespace": "",
            "role": "VALIDATOR",
            "schema": {
              "ID": "governance"
            },
            "who": {
              "NAME": "Owner"
            }
          }
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
