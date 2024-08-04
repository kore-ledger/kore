// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Helper service
//!

use tokio::sync::mpsc::Sender;
use network::CommandHelper as Command;
use crate::Error;

use super::NetworkMessage;


/// The Helper service.
#[derive(Debug, Clone)]
pub struct HelperService {
    /// The command sender to communicate with the worker.
    command_sender: Sender<Command<NetworkMessage>>,
}

impl HelperService {
    /// Create a new `HelperService`.
    pub fn new(command_sender: Sender<Command<NetworkMessage>>) -> Self {
        Self { command_sender }
    }

        /// Send command to the network worker.
        pub async fn send_command(&mut self, command: Command<NetworkMessage>) -> Result<(), Error> {
            self.command_sender
                .send(command)
                .await
                .map_err(|e| Error::Network(e.to_string()))
        }

    /// Send a message to the Helper worker.
    pub fn sender(&self) -> Sender<Command<NetworkMessage>> {
        self.command_sender.clone()
    }
}
