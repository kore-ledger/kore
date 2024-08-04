use std::fmt::format;

use crate::Error;

use super::{service::HelperService, NetworkMessage};
use borsh::error;
use network::Command as NetworkCommand;
use actor::SystemRef;
use network::CommandHelper as Command;
use rmp_serde::Deserializer;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::Cursor;
use identity::identifier::derive::KeyDerivator;
use tokio::sync::mpsc;
use network::{ PublicKeyEd25519, PublicKeysecp256k1, PublicKey, PeerId};

#[derive(Clone)]
pub struct Intermediary {
    /// Helpe service.
    service: HelperService,
    network_sender: mpsc::Sender<NetworkCommand>,
    derivator: KeyDerivator,
    system: SystemRef
}

impl Intermediary {
    pub fn new(
        network_sender: mpsc::Sender<NetworkCommand>,
        derivator: KeyDerivator,
        system: SystemRef,
    ) -> Self {
        // Create channels to communicate commands
        let (command_sender, command_receiver) = mpsc::channel(10000);

        let service = HelperService::new(command_sender);

        Self {
            service,
            derivator,
            network_sender,
            system
        }.spawn_command_receiver(command_receiver)
    }

    fn spawn_command_receiver(&mut self, mut command_receiver: mpsc::Receiver<Command<NetworkMessage>>) -> Self {
        let clone = self.clone();
        
        println!("spawn_command_receiver");
        tokio::spawn(async move {
            loop {
                println!("loop!!!!!");
                tokio::select! {
                    command = command_receiver.recv() => {
                        println!("comando!!!!!");
                        match command {
                            Some(command) => {
                                if let Err(error) = clone.handle_command(command).await {
                                    // Ver el error y actuar según TODO
                                };
                            }
                            None => {
                                break;
                            },
                        }
                    },
                }
            }
        });

        self.clone()
    }

    async fn handle_command(&self, command: Command<NetworkMessage>) -> Result<(), Error> {
        match command {
            Command::SendMessage { message } => {
                // Public key to peer_id
                let node_peer = Intermediary::to_peer_id(self.derivator, message.info.from.as_bytes())?;
                // Message to Vec<u8>
                let network_message = rmp_serde::to_vec(&message)
                .map_err(|error| Error::NetworkHelper(format!("{}", error)))?;
                // Send message to network
                if let Err(error) = self.network_sender.send(NetworkCommand::SendMessage { peer: node_peer, message: network_message }).await {
                    return Err(Error::Network(format!("Can not send message to network: {}", error)));
                };
            },
            Command::ReceivedMessage { message } => {
                let cur = Cursor::new(message);
                let mut de = Deserializer::new(cur);

                let message: NetworkMessage = match Deserialize::deserialize(&mut de) {
                    Ok(message) => message,
                    Err(e) => {
                        return Err(Error::NetworkHelper(format!("Can not deserialize message: {}", e)));
                    }
                };
                // Sacar el actor que proceda y enviar el respectivo mensaje al actor TODO:
            }
        }
        Ok(())
    }

    fn to_peer_id(derivator: KeyDerivator, peer: &[u8]) -> Result<PeerId, Error> {
         match derivator {
            KeyDerivator::Ed25519 => {
            if let Ok(public_key) = PublicKeyEd25519::try_from_bytes(peer) {
                let peer = PublicKey::from(public_key);
                Ok(peer.to_peer_id())
            } else {
                Err(Error::NetworkHelper(format!("Invalid Ed25519 public key, can not convert to PeerID")))
            }

            }, KeyDerivator::Secp256k1 => {
                if let Ok(public_key) = PublicKeysecp256k1::try_from_bytes(peer) {
                    let peer = PublicKey::from(public_key);
                    Ok(peer.to_peer_id())
                } else {
                    Err(Error::NetworkHelper(format!("Invalid Secp256k1 public key, can not convert to PeerID")))
                }
            }
            }
    }

    /// Send command to the network worker.
    pub async fn send_command(
        &mut self,
        command: Command<NetworkMessage>,
    ) -> Result<(), Error> {
        self.service
            .send_command(command)
            .await
            .map_err(|e| Error::NetworkHelper(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use actor::ActorSystem;
    use identity::identifier::derive::KeyDerivator;
    use tokio::sync::mpsc;

    use super::Intermediary;
}