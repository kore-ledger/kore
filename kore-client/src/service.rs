//

use crate::error::Error;
use super::{ClientMessage, ClientResponse};

use network::{Command as NetworkCommand, CommandHelper, PeerId};
use kore_common::{ActorMessage, EventRequest, NetworkMessage, RequestHandlerResponse, Signed};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use serde::Deserialize;
use rmp_serde::Deserializer;
use tracing::error;

use std::{
    io::Cursor,
    collections::HashMap,
};

#[derive(Clone)]
/// The Kore Client Service.
pub struct ClientService {
    peer_id: PeerId,
    network_sender: mpsc::Sender<NetworkCommand>,
    cancellation_token: CancellationToken,
}

impl ClientService {
    /// Create a new Client Service.
    pub fn build(
        peer_id: PeerId,
        network_sender: mpsc::Sender<NetworkCommand>,
        cancellation_token: CancellationToken,
    ) -> mpsc::Sender<CommandHelper<NetworkMessage>> {
        // Client service channel.
        let (client_sender, mut client_receiver) = mpsc::channel(100000);

        Self {
            peer_id,
            network_sender,
            cancellation_token,
        }
        .run(client_receiver);
        client_sender
    }

    fn run(
        &mut self, 
        mut client_receiver: mpsc::Receiver<CommandHelper<NetworkMessage>>
    ){
        let client_service = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(command) = client_receiver.recv() => {
                        // Handle the command.
                        if let Err(error) = client_service.handle_command(command).await {
                            error!("Error handling command: {}", error);
                        }
                    }
                    _ = client_service.cancellation_token.cancelled() => {
                        break;
                    }
                }
            }
        });
    }

    async fn handle_command(&self, command: CommandHelper<NetworkMessage>) -> Result<(), Error> {
        match command {
            CommandHelper::SendMessage { message } => {
                // Message to Vec<u8>
                let msg =
                    rmp_serde::to_vec(&message).map_err(|error| {
                        Error::Network(format!("{}", error))
                    })?;

                // Send message to network
                if let Err(error) = self
                    .network_sender
                    .send(NetworkCommand::SendMessage {
                        peer: self.peer_id,
                        message: msg,
                    })
                    .await
                {
                    return Err(Error::Network(format!(
                        "Can not send message to network: {}",
                        error
                    )));
                };
                Ok(())
            }
            CommandHelper::ReceivedMessage { message } => {
                // Deserialize message
                let cur = Cursor::new(message);
                let mut de = Deserializer::new(cur);
                let message: NetworkMessage =
                    match Deserialize::deserialize(&mut de) {
                        Ok(message) => message,
                        Err(e) => {
                            return Err(Error::Network(format!(
                                "Can not deserialize message: {}",
                                e
                            )));
                        }
                    };
                match message.message {
                    ActorMessage::EventRes { response } => {
                        // Here we would handle the response, e.g., notify waiting tasks.
                        // For now, we just log it.
                        println!("Received Event Response: {:?}", response);
                    }
                    _ => {
                        error!("Received unsupported message type");
                    }
                }
                
                Ok(())
            }
        }
    }

}
