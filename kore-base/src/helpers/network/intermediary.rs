// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    approval::approver::{Approver, ApproverMessage},
    distribution::distributor::{Distributor, DistributorMessage},
    evaluation::{
        evaluator::{Evaluator, EvaluatorMessage},
        schema::{EvaluationSchema, EvaluationSchemaMessage},
    },
    update::updater::{Updater, UpdaterMessage},
    validation::{
        schema::{ValidationSchema, ValidationSchemaMessage},
        validator::{Validator, ValidatorMessage},
    },
    Error,
};

use super::ActorMessage;
use super::{service::HelperService, NetworkMessage};
use actor::{ActorPath, ActorRef, Error as ActorError, SystemRef};
use identity::identifier::derive::KeyDerivator;
use network::Command as NetworkCommand;
use network::CommandHelper as Command;
use network::{PeerId, PublicKey, PublicKeyEd25519, PublicKeysecp256k1};
use rmp_serde::Deserializer;
use serde::Deserialize;
use std::io::Cursor;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::error;

const TARGET_NETWORK: &str = "Kore-Helper-Network";

#[derive(Clone)]
pub struct Intermediary {
    service: HelperService,
    network_sender: mpsc::Sender<NetworkCommand>,
    derivator: KeyDerivator,
    system: SystemRef,
    token: CancellationToken,
}

impl Intermediary {
    pub fn new(
        network_sender: mpsc::Sender<NetworkCommand>,
        derivator: KeyDerivator,
        system: SystemRef,
        token: CancellationToken,
    ) -> Self {
        let (command_sender, command_receiver) = mpsc::channel(10000);

        let service = HelperService::new(command_sender);

        Self {
            service,
            derivator,
            network_sender,
            system,
            token,
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
                        if let Some(command) = command{
                            if let Err(e) = clone.handle_command(command).await {
                                error!(TARGET_NETWORK, "{}", e);
                                if let Error::Network(_) = e {
                                    clone.token.cancel();
                                    break;
                                }
                            };
                        }
                    },
                    _ = clone.token.cancelled() => {
                        break;
                    }
                }
            }
        });

        self.clone()
    }

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
                    return Err(Error::Network(format!(
                        "Can not send message to network: {}",
                        error
                    )));
                };
            }
            Command::ReceivedMessage { message } => {
                let cur = Cursor::new(message);
                let mut de = Deserializer::new(cur);

                let message: NetworkMessage =
                    match Deserialize::deserialize(&mut de) {
                        Ok(message) => message,
                        Err(e) => {
                            return Err(Error::NetworkHelper(format!(
                                "Can not deserialize message: {}",
                                e
                            )));
                        }
                    };

                match message.message {
                    ActorMessage::DistributionGetLastSn { subject_id } => {
                        let distributor_path =
                            ActorPath::from(message.info.reciver_actor.clone());
                        let distributor_actor: Option<ActorRef<Distributor>> =
                            self.system.get_actor(&distributor_path).await;

                        if let Some(distributor_actor) = distributor_actor {
                            if let Err(e) = distributor_actor
                                .tell(DistributorMessage::GetLastSn {
                                    subject_id: subject_id.to_string(),
                                    info: message.info,
                                })
                                .await
                            {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not send a message to {}: {}",
                                    distributor_path, e
                                )));
                            };
                        } else {
                            return Err(Error::NetworkHelper(format!(
                                "Can not get Actor: {}",
                                distributor_path
                            )));
                        };
                    }
                    ActorMessage::AuthLastSn { sn } => {
                        let authorizer_path =
                            ActorPath::from(message.info.reciver_actor.clone());
                        let authorizer_actor: Option<ActorRef<Updater>> =
                            self.system.get_actor(&authorizer_path).await;

                        if let Some(authorizer_actor) = authorizer_actor {
                            if let Err(e) = authorizer_actor
                                .tell(UpdaterMessage::NetworkResponse { sn })
                                .await
                            {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not send a message to {}: {}",
                                    authorizer_path, e
                                )));
                            };
                        } else {
                            return Err(Error::NetworkHelper(format!(
                                "Can not get Actor: {}",
                                authorizer_path
                            )));
                        };
                    }
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
                                if let Err(e) = validator_actor
                                    .tell(ValidatorMessage::NetworkRequest {
                                        validation_req: req,
                                        info: message.info,
                                    })
                                    .await
                                {
                                    return Err(Error::NetworkHelper(format!(
                                        "Can not send a message to {}: {}",
                                        validator_path, e
                                    )));
                                };
                            } else {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not get Actor: {}",
                                    validator_path
                                )));
                            };
                        } else {
                            let validator_actor: Option<
                                ActorRef<ValidationSchema>,
                            > = self.system.get_actor(&validator_path).await;

                            // We obtain the validator
                            if let Some(validator_actor) = validator_actor {
                                if let Err(e) = validator_actor
                                    .tell(ValidationSchemaMessage::NetworkRequest {
                                        validation_req: req,
                                        info: message.info,
                                    })
                                    .await
                                    {
                                        return Err(Error::NetworkHelper(format!("Can not send a message to {}: {}",validator_path, e)));
                                    };
                            } else {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not get Actor: {}",
                                    validator_path
                                )));
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
                                if let Err(e) = evaluator_actor
                                    .tell(EvaluatorMessage::NetworkRequest {
                                        evaluation_req: req,
                                        info: message.info,
                                    })
                                    .await
                                {
                                    return Err(Error::NetworkHelper(format!(
                                        "Can not send a message to {}: {}",
                                        evaluator_path, e
                                    )));
                                };
                            } else {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not get Actor: {}",
                                    evaluator_path
                                )));
                            };
                        } else {
                            // Evaluator actor.
                            let evaluator_actor: Option<
                                ActorRef<EvaluationSchema>,
                            > = self.system.get_actor(&evaluator_path).await;

                            // We obtain the validator
                            if let Some(evaluator_actor) = evaluator_actor {
                                if let Err(e) = evaluator_actor
                            .tell(EvaluationSchemaMessage::NetworkRequest {
                                evaluation_req: req,
                                info: message.info,
                            })
                            .await
                            {
                                return Err(Error::NetworkHelper(format!("Can not send a message to {}: {}",evaluator_path, e)));
                            };
                            } else {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not get Actor: {}",
                                    evaluator_path
                                )));
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
                            if let Err(e) = approver_actor
                                .tell(ApproverMessage::NetworkRequest {
                                    approval_req: req,
                                    info: message.info,
                                })
                                .await
                            {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not send a message to {}: {}",
                                    approver_path, e
                                )));
                            };
                        } else {
                            return Err(Error::NetworkHelper(format!(
                                "Can not get Actor: {}",
                                approver_path
                            )));
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
                                return Err(Error::NetworkHelper(format!(
                                    "Can not get Actor: {}",
                                    node_distributor_path
                                )));
                            }
                        };

                        // We obtain the validator
                        if let Err(e) = distributor_actor
                            .tell(DistributorMessage::LastEventDistribution {
                                event,
                                ledger,
                                info: message.info,
                            })
                            .await
                        {
                            return Err(Error::NetworkHelper(format!(
                                "Can not send a message to {}: {}",
                                distributor_actor.path(),
                                e
                            )));
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
                            if let Err(e) = distributor_actor
                                .tell(DistributorMessage::SendDistribution {
                                    gov_version,
                                    actual_sn,
                                    subject_id: subject_id.to_string(),
                                    info: message.info,
                                })
                                .await
                            {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not send a message to {}: {}",
                                    distributor_path, e
                                )));
                            };
                        } else {
                            return Err(Error::NetworkHelper(format!(
                                "Can not get Actor: {}",
                                distributor_path
                            )));
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
                            if let Err(e) = validator_actor
                                .tell(ValidatorMessage::NetworkResponse {
                                    validation_res: res,
                                    request_id: message.info.request_id,
                                })
                                .await
                            {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not send a message to {}: {}",
                                    validator_path, e
                                )));
                            };
                        } else {
                            return Err(Error::NetworkHelper(format!(
                                "Can not get Actor: {}",
                                validator_path
                            )));
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
                            if let Err(e) = evaluator_actor
                                .tell(EvaluatorMessage::NetworkResponse {
                                    evaluation_res: res,
                                    request_id: message.info.request_id,
                                })
                                .await
                            {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not send a message to {}: {}",
                                    evaluator_path, e
                                )));
                            };
                        } else {
                            return Err(Error::NetworkHelper(format!(
                                "Can not get Actor: {}",
                                evaluator_path
                            )));
                        }
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
                            if let Err(e) = approver_actor
                                .tell(ApproverMessage::NetworkResponse {
                                    approval_res: *res,
                                    request_id: message.info.request_id,
                                })
                                .await
                            {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not send a message to {}: {}",
                                    approver_path, e
                                )));
                            };
                        } else {
                            return Err(Error::NetworkHelper(format!(
                                "Can not get Actor: {}",
                                approver_path
                            )));
                        }
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
                                return Err(Error::NetworkHelper(format!(
                                    "Can not get Actor: {}",
                                    node_distributor_path
                                )));
                            }
                        };

                        // We obtain the validator
                        if let Err(e) = distributor_actor
                            .tell(DistributorMessage::LedgerDistribution {
                                events: ledger,
                                last_event,
                                info: message.info,
                            })
                            .await
                        {
                            return Err(Error::NetworkHelper(format!(
                                "Can not send a message to {}: {}",
                                distributor_actor.path(),
                                e
                            )));
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
                            if let Err(e) = evaluator_actor
                                .tell(DistributorMessage::NetworkResponse {
                                    signer,
                                })
                                .await
                            {
                                return Err(Error::NetworkHelper(format!(
                                    "Can not send a message to {}: {}",
                                    distributor_path, e
                                )));
                            };
                        } else {
                            return Err(Error::NetworkHelper(format!(
                                "Can not get Actor: {}",
                                distributor_path
                            )));
                        }
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
    ) -> Result<(), ActorError> {
        self.service
            .send_command(command)
            .await
            .map_err(|e| ActorError::Functional(e.to_string()))
    }
}

#[cfg(test)]
mod tests {}
