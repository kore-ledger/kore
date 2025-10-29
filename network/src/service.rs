

//! # Network service
//!

use crate::{Command, Error};

use tokio::sync::mpsc::Sender;

/// The network service.
#[derive(Debug, Clone)]
pub struct NetworkService {
    /// The command sender to communicate with the worker.
    command_sender: Sender<Command>,
}

impl NetworkService {
    /// Create a new `NetworkService`.
    pub fn new(command_sender: Sender<Command>) -> Self {
        Self { command_sender }
    }

    /// Send command to the network worker.
    pub async fn send_command(
        &mut self,
        command: Command,
    ) -> Result<(), Error> {
        self.command_sender
            .send(command)
            .await
            .map_err(|e| Error::Command(e.to_string()))
    }

    /// Send a message to the network worker.
    pub fn sender(&self) -> Sender<Command> {
        self.command_sender.clone()
    }
}
