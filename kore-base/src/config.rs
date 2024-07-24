// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Configuration module

use identity::identifier::derive::{digest::DigestDerivator, KeyDerivator};
use serde::Deserialize;

/// Node configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    /// Key derivator.
    key_derivator: KeyDerivator,
    /// Digest derivator.
    digest_derivator: DigestDerivator,
    /// Database configuration.
    database: DbConfig,
}

impl Config {
    /// Creates a new `Config`.
    pub fn new(path: &str) -> Self {
        Self {
            key_derivator: KeyDerivator::Ed25519,
            digest_derivator: DigestDerivator::Blake3_256,
            database: DbConfig::Rocksdb {
                path: path.to_owned(),
            },
        }
    }
    /// Sets the key derivator.
    pub fn with_key_derivator(mut self, key_derivator: KeyDerivator) -> Self {
        self.key_derivator = key_derivator;
        self
    }
    /// Sets the digest derivator.
    pub fn with_digest_derivator(mut self, digest_derivator: DigestDerivator) -> Self {
        self.digest_derivator = digest_derivator;
        self
    }
    /// Sets the database configuration.
    pub fn with_database(mut self, database: DbConfig) -> Self {
        self.database = database;
        self
    }
    /// Returns the key derivator.
    pub fn get_key_derivator(&self) -> KeyDerivator {
        self.key_derivator
    }
    /// Returns the digest derivator.
    pub fn get_digest_derivator(&self) -> DigestDerivator {
        self.digest_derivator
    }
    /// Returns the database configuration.
    pub fn get_database(&self) -> &DbConfig {
        &self.database
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
