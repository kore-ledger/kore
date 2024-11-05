// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Configuration module

use identity::identifier::derive::{digest::DigestDerivator, KeyDerivator};
use serde::Deserialize;

/// Node configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    /// Key derivator.
    pub key_derivator: KeyDerivator,
    /// Digest derivator.
    pub digest_derivator: DigestDerivator,
    /// Database configuration.
    pub kore_db: DbConfig,

    pub external_db: String,
}

impl Config {
    /// Creates a new `Config`.
    pub fn new(kore_db_path: &str, external_db_path: &str) -> Self {
        Self {
            key_derivator: KeyDerivator::Ed25519,
            digest_derivator: DigestDerivator::Blake3_256,
            kore_db: DbConfig::Rocksdb {
                path: kore_db_path.to_owned(),
            },
            external_db: external_db_path.to_owned(),
        }
    }
}

/// Database configuration.
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
