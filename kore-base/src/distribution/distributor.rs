use std::time::Duration;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError,
    FixedIntervalStrategy, Handler, Message, RetryActor, RetryMessage,
    Strategy,
};
use async_trait::async_trait;
use identity::{
    identifier::{DigestIdentifier, KeyIdentifier},
    keys::KeyPair,
};
use network::ComunicateInfo;

use crate::{
    governance::model::Roles,
    model::{event::Ledger, network::RetryNetwork, Namespace},
    ActorMessage, Error, Event as KoreEvent, EventRequest, Governance,
    NetworkMessage, Node, NodeMessage, NodeResponse, Signed, Subject,
    SubjectCommand, SubjectResponse,
};

use super::Distribution;

pub enum EventTypes {
    Ledger(Ledger),
    Event(KoreEvent),
}

impl EventTypes {
    pub fn get_subject_id(&self) -> DigestIdentifier {
        match self {
            EventTypes::Ledger(ledger) => ledger.subject_id.clone(),
            EventTypes::Event(event) => event.subject_id.clone(),
        }
    }

    pub fn get_event_request(&self) -> EventRequest {
        match self {
            EventTypes::Ledger(ledger) => ledger.event_request.content.clone(),
            EventTypes::Event(event) => event.event_request.content.clone(),
        }
    }
}

pub struct Distributor {}

impl Distributor {
    async fn check_first_event(
        &self,
        ctx: &mut ActorContext<Distributor>,
        event: EventTypes,
        signer: KeyIdentifier,
        schema: &str,
    ) -> Result<bool, Error> {
        // TODO verificar la versión de la governanza

        let subject_id = event.get_subject_id();
        // path del sujeto
        let subject_path =
            ActorPath::from(format!("/user/node/{}", subject_id));

        // Sujeto
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        // Si el sujeto existe.
        let (namespace, create) = if let Some(subject_actor) = subject_actor {
            let response = match subject_actor
                .ask(SubjectCommand::GetSubjectMetadata)
                .await
            {
                Ok(response) => response,
                Err(e) => todo!(),
            };

            let metadata = match response {
                SubjectResponse::SubjectMetadata(metadata) => metadata,
                _ => todo!(),
            };
            (metadata.namespace.to_string(), false)
            // Si el sujeto no existe.
        } else {
            // El primero evento tiene que ser de creación sí o sí
            if let EventRequest::Create(request) = event.get_event_request() {
                (request.namespace, true)
            } else {
                // Error, no existe el sujeto y el primer evento no es de creación.
                todo!()
            }
        };

        // SI es una gov ver si la aceptamos,
        if schema == "governance" {
            // SI nada falla
            if let Ok(know) =
                self.authorized_gov(ctx, &subject_id.to_string()).await
            {
                // SI la governanza no la conocemos, por ende no está autorizada.
                if !know {
                    todo!()
                }
            } else {
                todo!()
            };
        }
        // si no es una gov ver si el signer es creator.
        else {
            let gov = self.get_gov(ctx, subject_id.clone()).await;
            let gov = match gov {
                Ok(gov) => gov,
                Err(e) => todo!(),
            };
            let creators = gov.get_signers(
                Roles::CREATOR { quantity: 0 },
                schema,
                Namespace::from(namespace),
            );
            if !creators.iter().any(|x| x.clone() == signer) {
                todo!()
            };
        }

        Ok(create)
    }

    async fn update_subject(
        &self,
        ctx: &mut ActorContext<Distributor>,
        events: Vec<Signed<KoreEvent>>,
    ) -> Result<(), Error> {
        let subject_id = events[0].content.subject_id.clone();

        let subject_path =
            ActorPath::from(format!("/user/node/{}", subject_id));
        let subject_actor: ActorRef<Subject> = if let Some(subject_actor) =
            ctx.system().get_actor(&subject_path).await
        {
            subject_actor
        } else {
            return Err(Error::Actor(format!(
                "The subject actor was not found in the expected path {}",
                subject_path
            )));
        };

        // Verificar firmas de cada evento, que el owner sea el que firmo el Event.
        // recorrer el vector hasta que
        for event in events {
            if subject_id != event.content.subject_id {
                todo!()
            }
        }
        Ok(())
    }

    async fn create_subject(
        &self,
        ctx: &mut ActorContext<Distributor>,
        ledger: Ledger,
        subject_keys: Option<KeyPair>,
    ) -> Result<(), Error> {
        let subject_keys = if let Some(subject_keys) = subject_keys {
            subject_keys
        } else {
            todo!()
        };

        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        let response = if let Some(node_actor) = node_actor {
            let response = node_actor
                .ask(NodeMessage::CreateNewSubject(ledger, subject_keys))
                .await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a node {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The node actor was not found in the expected path {}",
                node_path
            )));
        };
        match response {
            NodeResponse::SonWasCreated => Ok(()),
            NodeResponse::Error(e) => Err(e),
            _ => Err(Error::Actor(
                "An unexpected response has been received from node actor"
                    .to_owned(),
            )),
        }
    }

    async fn authorized_gov(
        &self,
        ctx: &mut ActorContext<Distributor>,
        subject_id: &str,
    ) -> Result<bool, Error> {
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        let response = if let Some(node_actor) = node_actor {
            let response = node_actor
                .ask(NodeMessage::IKnowThisGov(subject_id.to_owned()))
                .await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a node {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The node actor was not found in the expected path {}",
                node_path
            )));
        };
        match response {
            NodeResponse::IKnowThisGov(know) => Ok(know),
            _ => Err(Error::Actor(
                "An unexpected response has been received from node actor"
                    .to_owned(),
            )),
        }
    }

    async fn get_gov(
        &self,
        ctx: &mut ActorContext<Distributor>,
        subject_id: DigestIdentifier,
    ) -> Result<Governance, Error> {
        // Governance path
        let governance_path =
            ActorPath::from(format!("/user/node/{}", subject_id));
        // Governance actor.
        let governance_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&governance_path).await;

        // We obtain the actor governance
        let response = if let Some(governance_actor) = governance_actor {
            // We ask a governance
            let response =
                governance_actor.ask(SubjectCommand::GetGovernance).await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a Subject {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The governance actor was not found in the expected path {}",
                governance_path
            )));
        };

        match response {
            SubjectResponse::Governance(gov) => Ok(gov),
            SubjectResponse::Error(error) => Err(Error::Actor(format!(
                "The subject encountered problems when getting governance: {}",
                error
            ))),
            _ => Err(Error::Actor(
                "An unexpected response has been received from subject actor"
                    .to_owned(),
            )),
        }
    }
}

#[async_trait]
impl Actor for Distributor {
    type Event = ();
    type Message = DistributorCommand;
    type Response = ();
}

#[derive(Debug, Clone)]
pub enum DistributorCommand {
    // Un nodo nos solicitó la copia del ledger.
    SendDistribution,
    // Enviar a un nodo la replicación.
    NetworkDistribution {
        event: Signed<KoreEvent>,
        subject_keys: Option<KeyPair>,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    // El nodo al que le enviamos la replica la recivió, parar los reintentos.
    NetworkResponse,
    // Nos llega una replica, guardarla en informar que la hemos recivido
    LastEventDistribution {
        event: Signed<KoreEvent>,
        subject_keys: Option<KeyPair>,
        info: ComunicateInfo,
    },
    LedgerDistribution {
        events: Vec<Signed<Ledger>>,
        subject_keys: Option<KeyPair>,
        last_event: Option<Signed<KoreEvent>>,
        info: ComunicateInfo,
    },
}

impl Message for DistributorCommand {}

#[async_trait]
impl Handler<Distributor> for Distributor {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: DistributorCommand,
        ctx: &mut ActorContext<Distributor>,
    ) -> Result<(), ActorError> {
        match msg {
            DistributorCommand::SendDistribution => todo!(),
            DistributorCommand::NetworkDistribution {
                event,
                subject_keys,
                node_key,
                our_key,
            } => {
                let reciver_actor = format!(
                    "/user/node/{}/distributor",
                    event.content.subject_id
                );

                let message = NetworkMessage {
                    info: ComunicateInfo {
                        request_id: "".to_owned(),
                        sender: our_key,
                        reciver: node_key,
                        reciver_actor,
                        schema: "".to_owned(),
                    },
                    message: ActorMessage::DistributionLastEventReq(
                        event,
                        subject_keys,
                    ),
                };

                let target = RetryNetwork::default();

                let strategy = Strategy::FixedInterval(
                    FixedIntervalStrategy::new(3, Duration::from_secs(3)),
                );

                let retry_actor = RetryActor::new(target, message, strategy);

                let retry = if let Ok(retry) = ctx
                    .create_child::<RetryActor<RetryNetwork>>(
                        "retry",
                        retry_actor,
                    )
                    .await
                {
                    retry
                } else {
                    todo!()
                };

                if let Err(e) = retry.tell(RetryMessage::Retry).await {
                    todo!()
                };
            }
            DistributorCommand::NetworkResponse => todo!(),
            DistributorCommand::LastEventDistribution {
                event,
                subject_keys,
                info,
            } => {
                let event_type = EventTypes::Event(event.content.clone());
                let new_subject = match self
                    .check_first_event(
                        ctx,
                        event_type,
                        event.signature.signer.clone(),
                        &info.schema,
                    )
                    .await
                {
                    Ok(new) => new,
                    Err(e) => todo!(),
                };
                // Llegados a este punto hemos verificado si es la primera copia o no,
                // Si es una gobernanza y está authorizada o el firmante del evento tiene el rol
                // de creator.

                // Ahora hay que crear el sujeto si no existe sn = 0, o aplicar los eventos
                // verificando los hashes y aplicando el patch.

                if new_subject {
                    // Creamos el sujeto.
                    let ledger = Ledger::from(event.content);
                    if let Err(e) =
                        self.create_subject(ctx, ledger, subject_keys).await
                    {
                        todo!()
                    };
                } else {
                    // Verificar firmas del evento, el que lo envió es el owner del subject que se va a modificar.
                    // Checkear hashes, y apply patchs
                    // Actualizamos el sujeto.
                    todo!()
                }
            }
            DistributorCommand::LedgerDistribution {
                mut events,
                info,
                last_event,
                subject_keys,
            } => {
                if events.is_empty() {
                    todo!()
                }

                let event_type = EventTypes::Ledger(events[0].content.clone());
                let new_subject = match self
                    .check_first_event(
                        ctx,
                        event_type,
                        events[0].signature.signer.clone(),
                        &info.schema,
                    )
                    .await
                {
                    Ok(new) => new,
                    Err(e) => todo!(),
                };

                if new_subject {
                    // Creamos el sujeto.
                    if let Err(e) = self
                        .create_subject(
                            ctx,
                            events[0].content.clone(),
                            subject_keys,
                        )
                        .await
                    {
                        todo!()
                    };

                    let _ = events.remove(0);
                    if events.is_empty() {
                        // solo había un evento de creación, terminar TODO
                        todo!()
                    }
                }
                // Verificar firmas del evento, el que lo envió es el owner del subject que se va a modificar.
                // Checkear hashes, y apply patchs
                // Actualizamos el sujeto.
            }
        };

        Ok(())
    }

    // Realmente nos interesa manejar el máximo número de reintentos¿? TODO
    async fn on_child_error(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Distributor>,
    ) {
        if let ActorError::Functional(error) = error {
            if &error == "Max retries reached." {
                let replication_path = ctx.path().parent();

                // Replication actor.
                let replication_actor: Option<ActorRef<Distribution>> =
                    ctx.system().get_actor(&replication_path).await;

                if let Some(replication_actor) = replication_actor {
                    //TODO
                } else {
                    // TODO no se puede obtener evaluation! Parar.
                    // Can not obtain parent actor
                    // return Err(ActorError::Exists(evaluation_path));
                }
            }
        }
    }
}
