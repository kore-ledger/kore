use crate::{
    approval::approver::{Approver, ApproverMessage},
    distribution::distributor::{Distributor, DistributorMessage},
    evaluation::{
        evaluator::{Evaluator, EvaluatorMessage},
        schema::{EvaluationSchema, EvaluationSchemaMessage},
    },
    validation::{
        schema::{ValidationSchema, ValidationSchemaMessage},
        validator::{Validator, ValidatorMessage},
    },
    Error,
};

use super::ActorMessage;
use super::{service::HelperService, NetworkMessage};
use actor::{ActorPath, ActorRef, SystemRef};
use identity::identifier::derive::KeyDerivator;
use network::Command as NetworkCommand;
use network::CommandHelper as Command;
use network::{PeerId, PublicKey, PublicKeyEd25519, PublicKeysecp256k1};
use rmp_serde::Deserializer;
use serde::Deserialize;
use std::io::Cursor;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct Intermediary {
    /// Helpe service.
    service: HelperService,
    network_sender: mpsc::Sender<NetworkCommand>,
    derivator: KeyDerivator,
    system: SystemRef,
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
            system,
        }
        .spawn_command_receiver(command_receiver)
    }

    pub fn service(&self) -> HelperService {
        self.service.clone()
    }

    fn spawn_command_receiver(
        &mut self,
        mut command_receiver: mpsc::Receiver<Command<NetworkMessage>>,
    ) -> Self {
        let clone = self.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    command = command_receiver.recv() => {
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

    // TODO: hay que ver los errores, si se espera un actor que no se encuentra no hay que tumbar el nodo
    // podríamos recibir el ataque de otro nodo a un actor que no existe y no tendríamos por qué fallar
    // Habría que ver si tendríamos que responder si quiera.
    async fn handle_command(
        &self,
        command: Command<NetworkMessage>,
    ) -> Result<(), Error> {
        match command {
            Command::SendMessage { message } => {
                // Public key to peer_id
                let node_peer = Intermediary::to_peer_id(
                    self.derivator,
                    message.info.reciver.public_key.as_slice(),
                )?;
                // Message to Vec<u8>
                let network_message =
                    rmp_serde::to_vec(&message).map_err(|error| {
                        Error::NetworkHelper(format!("{}", error))
                    })?;
                // Send message to network
                if let Err(error) = self
                    .network_sender
                    .send(NetworkCommand::SendMessage {
                        peer: node_peer,
                        message: network_message,
                    })
                    .await
                {
                    todo!()
                    //return Err(Error::Network(format!("Can not send message to network: {}",error)));
                };
            }
            Command::ReceivedMessage { message } => {
                let cur = Cursor::new(message);
                let mut de = Deserializer::new(cur);

                let message: NetworkMessage =
                    match Deserialize::deserialize(&mut de) {
                        Ok(message) => message,
                        Err(_e) => {
                            todo!()
                            // return Err(Error::NetworkHelper(format!("Can not deserialize message: {}",e)));
                        }
                    };
                // Refactorizar esto TODO:
                match message.message {
                    ActorMessage::ValidationReq { req } => {
                        // Validator path.
                        let validator_path =
                            ActorPath::from(message.info.reciver_actor.clone());
                        // Validator actor.
                        if message.info.schema == "governance" {
                            let validator_actor: Option<ActorRef<Validator>> =
                                self.system.get_actor(&validator_path).await;

                            // We obtain the validator
                            if let Some(validator_actor) = validator_actor {
                                if let Err(error) = validator_actor
                                    .tell(ValidatorMessage::NetworkRequest {
                                        validation_req: req,
                                        info: message.info,
                                    })
                                    .await
                                {
                                    todo!()
                                    //return Err(Error::Actor(format!("Can not send a message to Validator Actor(Req): {}",error)));
                                };
                            } else {
                                todo!()
                                //return Err(Error::Actor(format!("The node actor was not found in the expected path {}",validator_path)));
                            };
                        } else {
                            let validator_actor: Option<
                                ActorRef<ValidationSchema>,
                            > = self.system.get_actor(&validator_path).await;

                            // We obtain the validator
                            if let Some(validator_actor) = validator_actor {
                                if let Err(error) = validator_actor
                                    .tell(ValidationSchemaMessage::NetworkRequest {
                                        validation_req: req,
                                        info: message.info,
                                    })
                                    .await
                                {
                                    todo!()
                                    // return Err(Error::Actor(format!("Can not send a message to Validator Actor(Req): {}",error)));
                                };
                            } else {
                                todo!()
                                //return Err(Error::Actor(format!("The node actor was not found in the expected path {}",validator_path)));
                            };
                        }
                    }
                    ActorMessage::EvaluationReq { req } => {
                        // Evaluator path.
                        let evaluator_path =
                            ActorPath::from(message.info.reciver_actor.clone());

                        if message.info.schema == "governance" {
                            // Evaluator actor.
                            let evaluator_actor: Option<ActorRef<Evaluator>> =
                                self.system.get_actor(&evaluator_path).await;

                            // We obtain the validator
                            if let Some(evaluator_actor) = evaluator_actor {
                                if let Err(error) = evaluator_actor
                                    .tell(EvaluatorMessage::NetworkRequest {
                                        evaluation_req: req,
                                        info: message.info,
                                    })
                                    .await
                                {
                                    todo!()
                                    // return Err(Error::Actor(format!("Can not send a message to Validator Actor(Req): {}",error)));
                                };
                            } else {
                                todo!()
                                //return Err(Error::Actor(format!("The node actor was not found in the expected path {}",evaluator_path)));
                            };
                        } else {
                            // Evaluator actor.
                            let evaluator_actor: Option<
                                ActorRef<EvaluationSchema>,
                            > = self.system.get_actor(&evaluator_path).await;

                            // We obtain the validator
                            if let Some(evaluator_actor) = evaluator_actor {
                                if let Err(error) = evaluator_actor
                            .tell(EvaluationSchemaMessage::NetworkRequest {
                                evaluation_req: req,
                                info: message.info,
                            })
                            .await
                        {
                            todo!()
                            //return Err(Error::Actor(format!("Can not send a message to Validator Actor(Req): {}",error)));
                        };
                            } else {
                                todo!()
                                //return Err(Error::Actor(format!("The node actor was not found in the expected path {}",evaluator_path)));
                            };
                        }
                    }
                    ActorMessage::ApprovalReq { req } => {
                        let approver_path =
                            ActorPath::from(message.info.reciver_actor.clone());

                        // Evaluator actor.
                        let approver_actor: Option<ActorRef<Approver>> =
                            self.system.get_actor(&approver_path).await;

                        // We obtain the validator
                        if let Some(approver_actor) = approver_actor {
                            if let Err(error) = approver_actor
                                .tell(ApproverMessage::NetworkRequest {
                                    approval_req: req,
                                    info: message.info,
                                })
                                .await
                            {
                                todo!()
                                // return Err(Error::Actor(format!("Can not send a message to Validator Actor(Req): {}",error)));
                            };
                        } else {
                            todo!()
                            //return Err(Error::Actor(format!("The node actor was not found in the expected path {}",evaluator_path)));
                        };
                    }
                    ActorMessage::DistributionLastEventReq {
                        event,
                        ledger,
                    } => {
                        // Distributor path.
                        let distributor_path =
                            ActorPath::from(message.info.reciver_actor.clone());

                        // SI ESTE sdistributor no está disponible quiere decir que el sujeto no existe, enviarlo al distributor del nodo
                        let distributor_actor: Option<ActorRef<Distributor>> =
                            self.system.get_actor(&distributor_path).await;

                        let distributor_actor = if let Some(distributor_actor) =
                            distributor_actor
                        {
                            distributor_actor
                        } else {
                            let node_distributor_path =
                                ActorPath::from("/user/node/distributor");
                            let node_distributor_actor: Option<
                                ActorRef<Distributor>,
                            > = self
                                .system
                                .get_actor(&node_distributor_path)
                                .await;
                            if let Some(node_distributor_actor) =
                                node_distributor_actor
                            {
                                node_distributor_actor
                            } else {
                                // Si este actor no está disponible hay un problema en el nodo TODO.
                                todo!()
                            }
                        };

                        // We obtain the validator
                        if let Err(error) = distributor_actor
                            .tell(DistributorMessage::LastEventDistribution {
                                event,
                                ledger,
                                info: message.info,
                            })
                            .await
                        {
                            todo!()
                            //return Err(Error::Actor(format!("Can not send a message to Validator Actor(Req): {}",error)));
                        };
                    }
                    ActorMessage::DistributionLedgerReq {
                        gov_version,
                        actual_sn,
                        subject_id,
                    } => {
                        let distributor_path =
                            ActorPath::from(message.info.reciver_actor.clone());
                        // Validator actor.
                        let distributor_actor: Option<ActorRef<Distributor>> =
                            self.system.get_actor(&distributor_path).await;

                        if let Some(distributor_actor) = distributor_actor {
                            if let Err(error) = distributor_actor
                                .tell(DistributorMessage::SendDistribution {
                                    gov_version,
                                    actual_sn,
                                    subject_id: subject_id.to_string(),
                                    info: message.info,
                                })
                                .await
                            {
                                todo!()
                                //return Err(Error::Actor(format!("Can not send a message to Evaluator Actor(Res): {}",error)));
                            };
                        } else {
                            todo!()
                            //return Err(Error::Actor(format!("The node actor was not found in the expected path {}",evaluator_path)));
                        };
                    }
                    ActorMessage::ValidationRes { res } => {
                        // Validator path.
                        let validator_path =
                            ActorPath::from(message.info.reciver_actor.clone());
                        // Validator actor.
                        let validator_actor: Option<ActorRef<Validator>> =
                            self.system.get_actor(&validator_path).await;

                        // We obtain the validator
                        if let Some(validator_actor) = validator_actor {
                            if let Err(error) = validator_actor
                                .tell(ValidatorMessage::NetworkResponse {
                                    validation_res: res,
                                    request_id: message.info.request_id,
                                })
                                .await
                            {
                                todo!()
                                //return Err(Error::Actor(format!("Can not send a message to Validator Actor(Res): {}",error)));
                            };
                        } else {
                            todo!()
                            //return Err(Error::Actor(format!("The node actor was not found in the expected path {}",validator_path)));
                        };
                    }
                    ActorMessage::EvaluationRes { res } => {
                        // Validator path.
                        let evaluator_path =
                            ActorPath::from(message.info.reciver_actor.clone());
                        // Validator actor.
                        let evaluator_actor: Option<ActorRef<Evaluator>> =
                            self.system.get_actor(&evaluator_path).await;

                        // We obtain the validator
                        if let Some(evaluator_actor) = evaluator_actor {
                            if let Err(error) = evaluator_actor
                                .tell(EvaluatorMessage::NetworkResponse {
                                    evaluation_res: res,
                                    request_id: message.info.request_id,
                                })
                                .await
                            {
                                todo!()
                                //return Err(Error::Actor(format!("Can not send a message to Evaluator Actor(Res): {}",error)));
                            };
                        } else {
                            todo!()
                            //return Err(Error::Actor(format!("The node actor was not found in the expected path {}",evaluator_path)));
                        };
                    }
                    ActorMessage::ApprovalRes { res } => {
                        // Validator path.
                        let approver_path =
                            ActorPath::from(message.info.reciver_actor.clone());
                        // Validator actor.
                        let approver_actor: Option<ActorRef<Approver>> =
                            self.system.get_actor(&approver_path).await;

                        // We obtain the validator
                        if let Some(approver_actor) = approver_actor {
                            if let Err(error) = approver_actor
                                .tell(ApproverMessage::NetworkResponse {
                                    approval_res: *res,
                                    request_id: message.info.request_id,
                                })
                                .await
                            {
                                todo!()
                                //return Err(Error::Actor(format!("Can not send a message to Evaluator Actor(Res): {}",error)));
                            };
                        } else {
                            todo!()
                            //return Err(Error::Actor(format!("The node actor was not found in the expected path {}",evaluator_path)));
                        };
                    }
                    ActorMessage::DistributionLedgerRes {
                        ledger,
                        last_event,
                    } => {
                        // Distributor path.
                        let distributor_path =
                            ActorPath::from(message.info.reciver_actor.clone());

                        // SI ESTE sdistributor no está disponible quiere decir que el sujeto no existe, enviarlo al distributor del nodo
                        let distributor_actor: Option<ActorRef<Distributor>> =
                            self.system.get_actor(&distributor_path).await;

                        let distributor_actor = if let Some(distributor_actor) =
                            distributor_actor
                        {
                            distributor_actor
                        } else {
                            let node_distributor_path =
                                ActorPath::from("/user/node/distributor");
                            let node_distributor_actor: Option<
                                ActorRef<Distributor>,
                            > = self
                                .system
                                .get_actor(&node_distributor_path)
                                .await;
                            if let Some(node_distributor_actor) =
                                node_distributor_actor
                            {
                                node_distributor_actor
                            } else {
                                // Si este actor no está disponible hay un problema en el nodo TODO.
                                todo!()
                            }
                        };

                        // We obtain the validator
                        if let Err(error) = distributor_actor
                            .tell(DistributorMessage::LedgerDistribution {
                                events: ledger,
                                last_event,
                                info: message.info,
                            })
                            .await
                        {
                            todo!()
                            //return Err(Error::Actor(format!("Can not send a message to Validator Actor(Req): {}",error)));
                        };
                    }
                    ActorMessage::DistributionLastEventRes { signer } => {
                        // Validator path.
                        let distributor_path =
                            ActorPath::from(message.info.reciver_actor.clone());
                        // Validator actor.
                        let distributor_actor: Option<ActorRef<Distributor>> =
                            self.system.get_actor(&distributor_path).await;

                        // We obtain the validator
                        if let Some(evaluator_actor) = distributor_actor {
                            if let Err(error) = evaluator_actor
                                .tell(DistributorMessage::NetworkResponse {
                                    signer,
                                })
                                .await
                            {
                                todo!()
                                // return Err(Error::Actor(format!("Can not send a message to Distributor Actor(Res): {}",error)));
                            };
                        } else {
                            todo!()
                            // return Err(Error::Actor(format!("The node actor was not found in the expected path {}",evaluator_path)));
                        };
                    }
                }
            }
        }

        Ok(())
    }

    fn to_peer_id(
        derivator: KeyDerivator,
        peer: &[u8],
    ) -> Result<PeerId, Error> {
        match derivator {
            KeyDerivator::Ed25519 => {
                if let Ok(public_key) = PublicKeyEd25519::try_from_bytes(peer) {
                    let peer = PublicKey::from(public_key);
                    Ok(peer.to_peer_id())
                } else {
                    Err(Error::NetworkHelper(
                        "Invalid Ed25519 public key, can not convert to PeerID"
                            .to_owned(),
                    ))
                }
            }
            KeyDerivator::Secp256k1 => {
                if let Ok(public_key) = PublicKeysecp256k1::try_from_bytes(peer)
                {
                    let peer = PublicKey::from(public_key);
                    Ok(peer.to_peer_id())
                } else {
                    Err(Error::NetworkHelper("Invalid Secp256k1 public key, can not convert to PeerID".to_owned()))
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
mod tests {}
