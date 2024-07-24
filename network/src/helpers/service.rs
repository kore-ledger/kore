// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Helper service
//!

use tokio::sync::mpsc::Sender;

use super::{Command, Error};

/// The Helper service.
#[derive(Debug, Clone)]
pub struct HelperService {
    /// The command sender to communicate with the worker.
    command_sender: Sender<Command>,
}

impl HelperService {
    /// Create a new `HelperService`.
    pub fn new(command_sender: Sender<Command>) -> Self {
        Self { command_sender }
    }

        /// Send command to the network worker.
        pub async fn send_command(&mut self, command: Command) -> Result<(), Error> {
            self.command_sender
                .send(command)
                .await
                .map_err(|e| Error::Worker(e.to_string()))
        }

    /// Send a message to the Helper worker.
    pub fn sender(&self) -> Sender<Command> {
        self.command_sender.clone()
    }
}
