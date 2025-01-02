// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Configuration module

use std::time::Duration;

use identity::identifier::derive::{digest::DigestDerivator, KeyDerivator};
use network::Config as NetworkConfig;
use serde::{Deserialize, Deserializer};

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
    pub sink: String,
}

/// Database configuration.
#[derive(Debug, Clone, Deserialize, PartialEq)]
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

impl Default for KoreDbConfig {
    fn default() -> Self {
        #[cfg(feature = "rocksdb")]
        return KoreDbConfig::Rocksdb {
            path: "db/local/rockdb".to_owned(),
        };
        #[cfg(feature = "sqlite")]
        return KoreDbConfig::SQLite {
            path: "db/local/sqlite".to_owned(),
        };
    }
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

    pub fn deserialize_db<'de, D>(
        deserializer: D,
    ) -> Result<KoreDbConfig, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path: String = String::deserialize(deserializer)?;
        #[cfg(feature = "rocksdb")]
        return Ok(KoreDbConfig::Rocksdb { path });
        #[cfg(feature = "sqlite")]
        return Ok(DbSettings::Sqlite { path });
    }
}

/// Database configuration.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum ExternalDbConfig {
    /// SQLite database.
    #[cfg(feature = "ext-sqlite")]
    SQLite {
        /// Path to the database.
        path: String,
    },
}

impl Default for ExternalDbConfig {
    fn default() -> Self {
        #[cfg(feature = "ext-sqlite")]
        return ExternalDbConfig::SQLite {
            path: "db/ext/ext-sqlite".to_owned(),
        };
    }
}

impl ExternalDbConfig {
    pub fn build(path: &str) -> Self {
        #[cfg(feature = "ext-sqlite")]
        return ExternalDbConfig::SQLite {
            path: path.to_owned(),
        };
    }

    pub fn deserialize_db<'de, D>(
        deserializer: D,
    ) -> Result<ExternalDbConfig, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path: String = String::deserialize(deserializer)?;
        #[cfg(feature = "ext-sqlite")]
        return Ok(ExternalDbConfig::SQLite { path });
    }
}
