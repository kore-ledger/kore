// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Configuration module

use serde::Deserialize;

/// Node configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub database: DbConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub enum DbConfig {
    /// Rocksdb database.
    Rocksdb {
        /// Path to the database.
        path: String,
    },
    /// SQLite database.
    SQLite {
        /// Path to the database.
        path: String,
    },
}
