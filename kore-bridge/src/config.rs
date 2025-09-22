// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use kore_base::config::{Config as KoreConfig, Logging, SinkConfig};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    /// Settings from Kore Base.
    pub kore_config: KoreConfig,
    /// Path for encryptep keys.
    pub keys_path: String,
    /// TcpListener from prometheus axum server.
    pub prometheus: String,
    /// Logging parameters.
    pub logging: Logging,
    /// Sink parameters.
    pub sink: SinkConfig
}

impl Config {
    pub fn add_path(&mut self, path: &str) {
        self.keys_path = format!("{}/{}", path, self.keys_path);
        self.kore_config.add_path(path);
    }
}
