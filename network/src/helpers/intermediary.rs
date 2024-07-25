use super::{service::HelperService, Command, Error};
use crate::Command as NetworkCommand;
use actor::SystemRef;

use identity::identifier::derive::KeyDerivator;
use tokio::sync::mpsc;
use libp2p::{identity::{
    ed25519::PublicKey as PublicKeyEd25519, secp256k1::PublicKey as PublicKeysecp256k1, PublicKey
}, PeerId};

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

    fn spawn_command_receiver(&mut self, mut command_receiver: mpsc::Receiver<Command>) -> Self {
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
                                clone.clone().handle_command(command).await;
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

    async fn handle_command(self, command: Command) {
        println!("Recibido Command::SendMessage");
        match command {
            Command::SendMessage { peer, message } => {
                println!("{:?}", peer);
                let peer = Intermediary::to_peer_id(self.derivator, &peer).to_string();
                self.network_sender.send(NetworkCommand::SendMessage { peer, message }).await.unwrap();
                println!("Enviado NetworkCommand::SendMessage");
            }
        }
    }

    fn to_peer_id(derivator: KeyDerivator, peer: &[u8]) -> PeerId {
         match derivator {
            KeyDerivator::Ed25519 => {
            if let Ok(public_key) = PublicKeyEd25519::try_from_bytes(peer) {
                let peer = PublicKey::from(public_key);
                peer.to_peer_id()
            } else {
                println!("ELSE");
                PeerId::random()
            }

            }, KeyDerivator::Secp256k1 => {
                if let Ok(public_key) = PublicKeysecp256k1::try_from_bytes(peer) {
                    let peer = PublicKey::from(public_key);
                    peer.to_peer_id()
                } else {
                    panic!("");
                }
            }
            }
    }

    /// Send command to the network worker.
    pub async fn send_command(
        &mut self,
        command: Command,
    ) -> Result<(), Error> {
        self.service
            .send_command(command)
            .await
            .map_err(|e| Error::Worker(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use actor::ActorSystem;
    use identity::identifier::derive::KeyDerivator;
    use tokio::sync::mpsc;

    use crate::helpers::Command;

    use super::Intermediary;

    #[tokio::test]
    async fn prueba() {
        let (command_sender, command_receiver) = mpsc::channel(100);
            // Create de actor system.
        let (system, mut runner) = ActorSystem::create();
        let mut int = Intermediary::new(command_sender, KeyDerivator::Ed25519, system);

        let mut a = int.clone();
        println!("Iniciando Test");

        int.send_command(Command::SendMessage { peer: vec![1], message: vec![1] }).await.unwrap();
        int.send_command(Command::SendMessage { peer: vec![2], message: vec![2] }).await.unwrap();
        int.send_command(Command::SendMessage { peer: vec![3], message: vec![3] }).await.unwrap();

        a.send_command(Command::SendMessage { peer: vec![4], message: vec![1] }).await.unwrap();
        a.send_command(Command::SendMessage { peer: vec![5], message: vec![2] }).await.unwrap();
        a.send_command(Command::SendMessage { peer: vec![6], message: vec![3] }).await.unwrap();

        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}