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
use tracing::event;

use crate::{
    evaluation::schema,
    governance::model::Roles,
    intermediary::Intermediary,
    model::{
        common::{get_gov, get_metadata, update_event},
        event::Ledger,
        network::RetryNetwork,
        Namespace,
    },
    subject::{
        event::{LedgerEvent, LedgerEventMessage, LedgerEventResponse},
        SubjectMetadata,
    },
    ActorMessage, Error, Event as KoreEvent, EventRequest, Governance,
    NetworkMessage, Node, NodeMessage, NodeResponse, Signed, Subject,
    SubjectMessage, SubjectResponse,
};

use super::{Distribution, DistributionMessage};

enum CheckGovernance {
    Continue,
    Finish,
}

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

pub struct Distributor {
    pub node: KeyIdentifier,
}

impl Distributor {
    async fn check_first_event(
        &self,
        ctx: &mut ActorContext<Distributor>,
        event: EventTypes,
        signer: KeyIdentifier,
        schema: &str,
    ) -> Result<bool, Error> {
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
                .ask(SubjectMessage::GetSubjectMetadata)
                .await
            {
                Ok(response) => response,
                Err(e) => todo!(),
            };

            let metadata = match response {
                SubjectResponse::SubjectMetadata(metadata) => metadata,
                _ => todo!(),
            };
            (metadata.namespace, false)
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
            let gov = get_gov(ctx, subject_id.clone()).await;
            let gov = match gov {
                Ok(gov) => gov,
                Err(e) => todo!(),
            };
            let creators = gov.get_signers(
                Roles::CREATOR { quantity: 0 },
                schema,
                namespace,
            );
            if !creators.iter().any(|x| x.clone() == signer) {
                todo!()
            };
        }

        Ok(create)
    }

    async fn get_subject(
        &self,
        ctx: &mut ActorContext<Distributor>,
        subject_id: DigestIdentifier,
    ) -> Result<ActorRef<Subject>, Error> {
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

        Ok(subject_actor)
    }

    async fn create_subject(
        &self,
        ctx: &mut ActorContext<Distributor>,
        ledger: Signed<Ledger>,
    ) -> Result<(), Error> {
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        let response = if let Some(node_actor) = node_actor {
            let response = node_actor
                .ask(NodeMessage::CreateNewSubjectLedger(ledger))
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
                .ask(NodeMessage::IsAuthorized(subject_id.to_owned()))
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
            NodeResponse::IsAuthorized(know) => Ok(know),
            _ => Err(Error::Actor(
                "An unexpected response has been received from node actor"
                    .to_owned(),
            )),
        }
    }

    async fn get_ledger(
        &self,
        ctx: &mut ActorContext<Distributor>,
        subject_id: DigestIdentifier,
        last_sn: u64,
    ) -> Result<(Vec<Signed<Ledger>>, Option<Signed<KoreEvent>>), Error> {
        let subject_path =
            ActorPath::from(format!("/user/node/{}", subject_id));
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        let response = if let Some(subject_actor) = subject_actor {
            // We ask a node
            let response = subject_actor
                .ask(SubjectMessage::GetLedger { last_sn })
                .await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a subject {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The node actor was not found in the expected path {}",
                subject_path
            )));
        };

        match response {
            SubjectResponse::Ledger(data) => Ok(data),
            SubjectResponse::Error(e) => todo!(),
            _ => Err(Error::Actor(
                "An unexpected response has been received from subject actor"
                    .to_owned(),
            )),
        }
    }

    async fn check_gov_version(
        &self,
        ctx: &mut ActorContext<Distributor>,
        subject_id: DigestIdentifier,
        gov_version: Option<u64>,
        info: ComunicateInfo,
    ) -> Result<CheckGovernance, ActorError> {
        let gov = match get_gov(ctx, subject_id.clone()).await {
            Ok(gov) => gov,
            Err(e) => todo!(),
        };

        let metadata = match get_metadata(ctx, subject_id.clone()).await {
            Ok(metadata) => metadata,
            Err(e) => todo!(),
        };

        let our_gov_version = gov.version;

        if let Some(gov_version) = gov_version {
            // Comprobar versión de la gobernanza, si no es la misma le digo que se actualice o me actualizo.
            match our_gov_version.cmp(&gov_version) {
                std::cmp::Ordering::Less => {
                    let gov_id = if info.schema != "governance" {
                        metadata.governance_id
                    } else {
                        metadata.subject_id
                    };

                    // Mi version es menor, me actualizo. y no le envío nada
                    let new_info = ComunicateInfo {
                        reciver: info.sender,
                        sender: info.reciver,
                        request_id: info.request_id,
                        reciver_actor: format!(
                            "/user/node/{}/distributor",
                            gov_id
                        ),
                        schema: "governance".to_owned(),
                    };

                    let helper: Option<Intermediary> =
                        ctx.system().get_helper("NetworkIntermediary").await;

                    let mut helper = if let Some(helper) = helper {
                        helper
                    } else {
                        // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
                        // return Err(ActorError::Get("Error".to_owned()))
                        return Err(ActorError::NotHelper);
                    };

                    // TODO firmar la respuesta. Por ahora no, ya que si no tengo la gov en la última versión no puedo saber si es un testigo.
                    if let Err(e) = helper
                        .send_command(network::CommandHelper::SendMessage {
                            message: NetworkMessage {
                                info: new_info,
                                message: ActorMessage::DistributionLedgerReq {
                                    gov_version: Some(our_gov_version),
                                    actual_sn: Some(our_gov_version),
                                    subject_id: gov_id,
                                },
                            },
                        })
                        .await
                    {
                        todo!()
                        // error al enviar mensaje, propagar hacia arriba
                    };

                    return Ok(CheckGovernance::Finish);
                }
                std::cmp::Ordering::Equal => {}
                std::cmp::Ordering::Greater => {
                    // Su version es menor. Traslado la solicitud de actualización
                    // a mi distributor gov.
                    if info.schema != "governance" {
                        let new_info = ComunicateInfo {
                            reciver: info.reciver,
                            sender: info.sender,
                            request_id: info.request_id,
                            reciver_actor: format!(
                                "/user/node/{}/distributor",
                                metadata.governance_id
                            ),
                            schema: "governance".to_owned(),
                        };

                        let distributor_path = ActorPath::from(format!(
                            "/user/node/{}/distributor",
                            metadata.governance_id
                        ));
                        let distributor_actor: ActorRef<Distributor> =
                            if let Some(distributor_actor) =
                                ctx.system().get_actor(&distributor_path).await
                            {
                                distributor_actor
                            } else {
                                todo!()
                            };

                        if let Err(e) = distributor_actor
                            .tell(DistributorMessage::SendDistribution {
                                gov_version: Some(gov_version),
                                actual_sn: Some(gov_version),
                                subject_id: metadata.governance_id,
                                info: new_info,
                            })
                            .await
                        {
                            todo!()
                        }
                        return Ok(CheckGovernance::Finish);
                    }
                }
            }
        } else {
            if info.schema != "governance" {
                // Error me estás pidiendo el ledger para un sujeto que no es una governanza
                // Y no me pasas la versión de lagov
                todo!()
            }
        }

        if !gov
            .get_signers(
                Roles::WITNESS,
                &metadata.schema_id.clone(),
                metadata.namespace.clone(),
            )
            .iter()
            .any(|x| x.clone() == info.sender)
        {
            // Se podría dar que sí sea testigo pero que yo no tenga la última versión.
            todo!()
        };

        Ok(CheckGovernance::Continue)
    }
}

#[async_trait]
impl Actor for Distributor {
    type Event = ();
    type Message = DistributorMessage;
    type Response = ();
}

#[derive(Debug, Clone)]
pub enum DistributorMessage {
    // Un nodo nos solicitó la copia del ledger.
    SendDistribution {
        gov_version: Option<u64>,
        actual_sn: Option<u64>,
        subject_id: DigestIdentifier,
        info: ComunicateInfo,
    },
    // Enviar a un nodo la replicación.
    NetworkDistribution {
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    // El nodo al que le enviamos la replica la recivió, parar los reintentos.
    NetworkResponse {
        signer: KeyIdentifier,
    },
    // Nos llega una replica, guardarla en informar que la hemos recivido
    LastEventDistribution {
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>,
        info: ComunicateInfo,
    },
    LedgerDistribution {
        events: Vec<Signed<Ledger>>,
        last_event: Option<Signed<KoreEvent>>,
        info: ComunicateInfo,
    },
}

impl Message for DistributorMessage {}

#[async_trait]
impl Handler<Distributor> for Distributor {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: DistributorMessage,
        ctx: &mut ActorContext<Distributor>,
    ) -> Result<(), ActorError> {
        match msg {
            DistributorMessage::SendDistribution {
                actual_sn,
                info,
                gov_version,
                subject_id,
            } => {
                let result = self
                    .check_gov_version(
                        ctx,
                        subject_id.clone(),
                        gov_version,
                        info.clone(),
                    )
                    .await?;

                if let CheckGovernance::Finish = result {
                    return Ok(());
                };

                let sn = if let Some(actual_sn) = actual_sn {
                    actual_sn
                } else {
                    0
                };

                // Sacar eventos.
                let (ledger, last_event) =
                    match self.get_ledger(ctx, subject_id.clone(), sn).await {
                        Ok(res) => res,
                        Err(e) => todo!(),
                    };

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver,
                    request_id: info.request_id,
                    reciver_actor: format!(
                        "/user/node/{}/distributor",
                        subject_id
                    ),
                    schema: info.schema,
                };

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("NetworkIntermediary").await;

                let mut helper = if let Some(helper) = helper {
                    helper
                } else {
                    // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
                    // return Err(ActorError::Get("Error".to_owned()))
                    return Err(ActorError::NotHelper);
                };

                // TODO firmar la respuesta.
                if let Err(e) = helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::DistributionLedgerRes {
                                ledger,
                                last_event,
                            },
                        },
                    })
                    .await
                {
                    todo!()
                };
            }
            DistributorMessage::NetworkDistribution {
                event,
                node_key,
                our_key,
                ledger,
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
                    message: ActorMessage::DistributionLastEventReq {
                        ledger,
                        event,
                    },
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
            DistributorMessage::NetworkResponse { signer } => {
                if signer == self.node {
                    let distribution_path = ctx.path().parent();

                    let distribution_actor: Option<ActorRef<Distribution>> =
                        ctx.system().get_actor(&distribution_path).await;

                    if let Some(distribution_actor) = distribution_actor {
                        if let Err(e) = distribution_actor
                            .tell(DistributionMessage::Response {
                                sender: self.node.clone(),
                            })
                            .await
                        {
                            todo!()
                        }
                    } else {
                        todo!()
                    }

                    let retry = if let Some(retry) =
                        ctx.get_child::<RetryActor<RetryNetwork>>("retry").await
                    {
                        retry
                    } else {
                        todo!()
                    };
                    if let Err(e) = retry.tell(RetryMessage::End).await {
                        todo!()
                    };
                    ctx.stop().await;
                }
            }
            DistributorMessage::LastEventDistribution {
                event,
                ledger,
                info,
            } => {
                let result = self
                    .check_gov_version(
                        ctx,
                        ledger.content.subject_id.clone(),
                        Some(ledger.content.gov_version.clone()),
                        info.clone(),
                    )
                    .await?;
                if let CheckGovernance::Finish = result {
                    return Ok(());
                };

                let event_type = EventTypes::Event(event.content.clone());
                // TODO verificar la versión de la governanza
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
                    if let Err(e) = self.create_subject(ctx, ledger).await {
                        todo!()
                    };
                } else {
                    let subject_ref = match self
                        .get_subject(ctx, ledger.content.subject_id.clone())
                        .await
                    {
                        Ok(subject) => subject,
                        Err(e) => todo!(),
                    };

                    let response = match subject_ref
                        .ask(SubjectMessage::UpdateLedger {
                            events: vec![ledger.clone()],
                        })
                        .await
                    {
                        Ok(res) => res,
                        Err(e) => todo!(),
                    };

                    match response {
                        SubjectResponse::LastSn(last_sn) => {
                            // NO se aplicó el evento porque tendría un sn demasiado grande, no es el que toca o ya está aplicado.
                            // Si fue demasiado grande
                            if last_sn < ledger.content.sn {
                                let gov = match get_gov(
                                    ctx,
                                    ledger.content.subject_id.clone(),
                                )
                                .await
                                {
                                    Ok(gov) => gov,
                                    Err(e) => todo!(),
                                };

                                let our_gov_version = gov.version;

                                let new_info = ComunicateInfo {
                                    reciver: info.sender,
                                    sender: info.reciver,
                                    request_id: info.request_id,
                                    reciver_actor: format!(
                                        "/user/node/{}/distributor",
                                        ledger.content.subject_id
                                    ),
                                    schema: info.schema,
                                };

                                let helper: Option<Intermediary> = ctx
                                    .system()
                                    .get_helper("NetworkIntermediary")
                                    .await;

                                let mut helper = if let Some(helper) = helper {
                                    helper
                                } else {
                                    // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
                                    // return Err(ActorError::Get("Error".to_owned()))
                                    return Err(ActorError::NotHelper);
                                };

                                // Pedimos copia del ledger.
                                if let Err(e) = helper.send_command(network::CommandHelper::SendMessage {
                                    message: NetworkMessage {
                                    info: new_info,
                                    message: ActorMessage::DistributionLedgerReq {
                                        gov_version: Some(our_gov_version),
                                        actual_sn: Some(last_sn),
                                        subject_id: ledger.content.subject_id,
                                    },
                                },
                            }).await {
                                todo!()
                                // error al enviar mensaje, propagar hacia arriba
                            };

                                return Ok(());
                            }
                        }
                        SubjectResponse::Error(e) => todo!(),
                        _ => todo!(),
                    };
                }

                if let Err(e) = update_event(ctx, event.clone()).await {
                    todo!()
                };

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    reciver_actor: format!(
                        "/user/node/{}/distribution/{}",
                        event.content.subject_id,
                        info.reciver.clone()
                    ),
                    schema: info.schema.clone(),
                };

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("NetworkIntermediary").await;

                let mut helper = if let Some(helper) = helper {
                    helper
                } else {
                    // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
                    // return Err(ActorError::Get("Error".to_owned()))
                    return Err(ActorError::NotHelper);
                };

                if let Err(e) = helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::DistributionLastEventRes {
                                signer: info.reciver,
                            },
                        },
                    })
                    .await
                {
                    todo!()
                };
            }
            DistributorMessage::LedgerDistribution {
                mut events,
                info,
                last_event,
            } => {
                if events.is_empty() {
                    todo!()
                }
                // TODO no voy a comparar governanzas, no creo que aquí haga falta, revizar en un futuro.

                let subject_id = events[0].content.subject_id.clone();

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
                    if let Err(e) =
                        self.create_subject(ctx, events[0].clone()).await
                    {
                        todo!()
                    };

                    let _ = events.remove(0);
                    if events.is_empty() {
                        // solo había un evento de creación, terminar
                        return Ok(());
                    }
                }

                let subject_ref = match self
                    .get_subject(ctx, events[0].content.subject_id.clone())
                    .await
                {
                    Ok(subject) => subject,
                    Err(e) => todo!(),
                };

                // Obtenemos el last_sn para saber si nos vale la pena intentar actualizar el ledger
                let response = match subject_ref
                    .ask(SubjectMessage::GetSubjectMetadata)
                    .await
                {
                    Ok(res) => res,
                    Err(e) => todo!(),
                };

                let metadata = match response {
                    SubjectResponse::SubjectMetadata(data) => data,
                    _ => todo!(),
                };

                let last_sn_events = if let Some(last_ledger) = events.last() {
                    last_ledger.content.sn
                } else {
                    todo!()
                };

                let last_sn = if last_sn_events > metadata.sn {
                    let response = match subject_ref
                        .ask(SubjectMessage::UpdateLedger { events })
                        .await
                    {
                        Ok(res) => res,
                        Err(e) => todo!(),
                    };

                    let last_sn = match response {
                        SubjectResponse::LastSn(last_sn) => last_sn,
                        SubjectResponse::Error(e) => todo!(),
                        _ => todo!(),
                    };
                    last_sn
                } else {
                    metadata.sn
                };

                if let Some(event) = last_event {
                    // si me envía esto quiere decir que ya no hay más eventos, terminó,
                    // Llegados a este punto este signed<Event> puede tener un sn inferior al mío.
                    // Entonces le tenemos que decir que se actualice.
                    // Si me actualicé ya no le digo nada más.
                    if last_sn > last_sn_events {
                        let our_path = ActorPath::from(format!(
                            "/user/node/{}/distributor",
                            subject_id
                        ));
                        let our_actor: Option<ActorRef<Distributor>> =
                            ctx.system().get_actor(&our_path).await;
                        if let Some(our_actor) = our_actor {
                            if let Err(e) = our_actor
                                .tell(DistributorMessage::SendDistribution {
                                    gov_version: Some(
                                        event.content.gov_version,
                                    ),
                                    actual_sn: Some(last_sn_events),
                                    subject_id,
                                    info,
                                })
                                .await
                            {
                                todo!()
                            }
                        } else {
                        }
                    } else if last_sn < last_sn_events {
                        if let Err(e) = update_event(ctx, event).await {
                            todo!()
                        };
                    }
                    return Ok(());
                };

                let gov_id = if info.schema == "governance" {
                    metadata.subject_id
                } else {
                    metadata.governance_id
                };

                let gov = match get_gov(ctx, gov_id).await {
                    Ok(gov) => gov,
                    Err(e) => todo!(),
                };

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    reciver_actor: format!(
                        "/user/node/{}/distributor",
                        subject_id
                    ),
                    schema: info.schema.clone(),
                };

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("NetworkIntermediary").await;
                let mut helper = if let Some(helper) = helper {
                    helper
                } else {
                    // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
                    // return Err(ActorError::Get("Error".to_owned()))
                    return Err(ActorError::NotHelper);
                };

                if let Err(e) = helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::DistributionLedgerReq {
                                gov_version: Some(gov.version),
                                actual_sn: Some(last_sn),
                                subject_id,
                            },
                        },
                    })
                    .await
                {
                    todo!()
                };
            }
        };

        Ok(())
    }

    async fn on_child_error(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Distributor>,
    ) {
        if let ActorError::Functional(error) = error {
            if &error == "Max retries reached." {
                let distribuiton_path = ctx.path().parent();

                // Replication actor.
                let distribuiton_actor: Option<ActorRef<Distribution>> =
                    ctx.system().get_actor(&distribuiton_path).await;

                if let Some(distribuiton_actor) = distribuiton_actor {
                    if let Err(e) = distribuiton_actor
                        .tell(DistributionMessage::Response {
                            sender: self.node.clone(),
                        })
                        .await
                    {
                        // TODO error, no se puede enviar la response
                        // return Err(e);
                    }
                } else {
                    // TODO no se puede obtener evaluation! Parar.
                    // Can not obtain parent actor
                    // return Err(ActorError::Exists(evaluation_path));
                }
            }
        }
    }
}
