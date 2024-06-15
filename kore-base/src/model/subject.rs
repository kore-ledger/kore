// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Subject module.
//! 
//! The `subject` module provides the `Subject` and `SubjectData` types, which represents a 
//! subject in the system.
//! 

use serde::{Deserialize, Serialize};

/// Subject struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Subject {
    /// The subject's id.
    pub id: String,
    /// The subject's data.
    pub data: SubjectData,
}

/// Subject data struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubjectData {
    /// The subject's name.
    pub name: String,
    /// The subject's age.
    pub age: u32,
}
