// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Configuration module

use std::time::Duration;

use identity::identifier::derive::{digest::DigestDerivator, KeyDerivator};
use network::Config as NetworkConfig;
use serde::Deserialize;

/// Node configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    /// Key derivator.
    pub key_derivator: KeyDerivator,
    /// Digest derivator.
    pub digest_derivator: DigestDerivator,
    /// Database configuration.
    pub kore_db: KoreDbConfig,
    /// External database configuration.
    pub external_db: ExternalDbConfig,
    /// Network configuration.
    pub network: NetworkConfig,
    /// Contract dir.
    pub contracts_dir: String,
    /// Approval mode.
    pub always_accept: bool,
    /// Garbage collector acts
    pub garbage_collector: Duration,
}

/// Database configuration.
#[derive(Debug, Clone, Deserialize)]
pub enum KoreDbConfig {
    /// Rocksdb database.
    #[cfg(feature = "rocksdb")]
    Rocksdb {
        /// Path to the database.
        path: String,
    },
    /// SQLite database.
    #[cfg(feature = "sqlite")]
    SQLite {
        /// Path to the database.
        path: String,
    },
}

impl KoreDbConfig {
    pub fn build(path: &str) -> Self {
        #[cfg(feature = "rocksdb")]
        return KoreDbConfig::Rocksdb {
            path: path.to_owned(),
        };
        #[cfg(feature = "sqlite")]
        return KoreDbConfig::SQLite {
            path: path.to_owned(),
        };
    }
}

/// Database configuration.
#[derive(Debug, Clone, Deserialize)]
pub enum ExternalDbConfig {
    /// SQLite database.
    SQLite {
        /// Path to the database.
        path: String,
    },
}

impl ExternalDbConfig {
    pub fn build(path: &str) -> Self {
        #[cfg(feature = "ext-sqlite")]
        return ExternalDbConfig::SQLite {
            path: path.to_owned(),
        };
    }
}
