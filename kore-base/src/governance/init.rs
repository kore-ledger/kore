// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Governance init state.
//! 

use crate::model::ValueWrapper;

pub fn init_state() -> ValueWrapper {
    ValueWrapper(serde_json::json!({
        "members": [],
        "roles": [
          {
            "namespace": "",
            "role": "WITNESS",
            "schema": {
                "ID": "governance"
            },
            "who": "MEMBERS"
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
