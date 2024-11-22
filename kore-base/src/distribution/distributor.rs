use std::time::Duration;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError,
    FixedIntervalStrategy, Handler, Message, RetryActor, RetryMessage,
    Strategy, SystemEvent,
};
use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use jsonschema::error;
use network::ComunicateInfo;

use crate::{
    governance::model::Roles,
    intermediary::Intermediary,
    model::{
        common::{get_gov, get_metadata, get_quantity, update_event},
        event::Ledger,
        network::RetryNetwork,
    },
    ActorMessage, Error, Event as KoreEvent, EventRequest, NetworkMessage,
    Node, NodeMessage, NodeResponse, Signed, Subject, SubjectMessage,
    SubjectResponse,
};

use super::{Distribution, DistributionMessage};

enum CheckGovernance {
    Continue,
    Finish,
}

pub struct Distributor {
    pub node: KeyIdentifier,
}

impl Distributor {
    async fn check_first_event(
        &self,
        ctx: &mut ActorContext<Distributor>,
        ledger: Ledger,
        our_key: KeyIdentifier,
        signer: KeyIdentifier,
        schema: &str,
    ) -> Result<bool, Error> {
        let subject_id = ledger.subject_id;

        let know = if let Ok(know) =
            self.authorized_subj(ctx, &subject_id.to_string()).await
        {
            know
        } else {
            todo!()
        };

        // path del sujeto
        let subject_path =
            ActorPath::from(format!("/user/node/{}", subject_id));

        // Sujeto
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        // Si el sujeto existe.
        let (namespace, create) = if let Some(subject_actor) = subject_actor {
            let response =
                match subject_actor.ask(SubjectMessage::GetMetadata).await {
                    Ok(response) => response,
                    Err(_e) => todo!(),
                };

            let metadata = match response {
                SubjectResponse::Metadata(metadata) => metadata,
                _ => todo!(),
            };
            (metadata.namespace, false)
            // Si el sujeto no existe.
        } else {
            // El primero evento tiene que ser de creación sí o sí
            if let EventRequest::Create(request) =
                ledger.event_request.content.clone()
            {
                (request.namespace, true)
            } else {
                // Error, no existe el sujeto y el primer evento no es de creación.
                todo!()
            }
        };

        if !know && schema != "governance" {
            let gov = if create {
                // El primero evento tiene que ser de creación sí o sí
                if let EventRequest::Create(request) =
                    ledger.event_request.content
                {
                    get_gov(ctx, &request.governance_id.to_string()).await
                } else {
                    // Error, no existe el sujeto y el primer evento no es de creación.
                    todo!()
                }
            } else {
                get_gov(ctx, &subject_id.to_string()).await
            };

            let gov = match gov {
                Ok(gov) => gov,
                Err(_e) => todo!(),
            };
            if !gov.has_this_role(
                &signer.to_string(),
                Roles::CREATOR { quantity: 0 },
                schema,
                namespace.clone(),
            ) {
                todo!()
            }

            if !gov.has_this_role(
                &our_key.to_string(),
                Roles::WITNESS,
                schema,
                namespace,
            ) {
                todo!()
            }
            // Comprobar que soy testigo para ese schema y esa governanza.
        } else if !know && schema == "governance" {
            // No hay que guardarla
            todo!()
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
        // TODO refactorizar.
        if let EventRequest::Create(request) =
            ledger.content.event_request.content.clone()
        {
            if request.schema_id != "governance" {
                let Ok(gov) =
                    get_gov(ctx, &request.governance_id.to_string()).await
                else {
                    todo!()
                };

                if let Some(max_quantity) = gov.max_creations(
                    &ledger.signature.signer.to_string(),
                    &request.schema_id,
                    request.namespace.clone(),
                ) {
                    let quantity = match get_quantity(
                        ctx,
                        request.governance_id.to_string(),
                        request.schema_id.clone(),
                        ledger.signature.signer.to_string(),
                        request.namespace.to_string(),
                    )
                    .await
                    {
                        Ok(quantity) => quantity,
                        Err(e) => todo!(),
                    };

                    if quantity >= max_quantity {
                        todo!()
                    }
                } else {
                    todo!()
                };
            }
        }

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

    async fn authorized_subj(
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
        subject_id: &str,
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

    async fn check_gov_version_ledger(
        ctx: &mut ActorContext<Distributor>,
        info: ComunicateInfo,
        gov_version: u64,
        event: EventRequest,
        subject_id: DigestIdentifier,
    ) -> Result<CheckGovernance, ActorError> {
        let (gov, gov_id) = if let EventRequest::Create(event_req) = event {
            let gov = get_gov(ctx, &event_req.governance_id.to_string()).await;
            (gov, event_req.governance_id)
        } else {
            let gov = get_gov(ctx, &subject_id.to_string()).await;
            let metadata =
                match get_metadata(ctx, &subject_id.to_string()).await {
                    Ok(metadata) => metadata,
                    Err(_e) => todo!(),
                };

            (gov, metadata.governance_id)
        };
        // SOlo a no governanzas, el sujeto puede no existir.

        let gov = match gov {
            Ok(gov) => gov,
            Err(_e) => todo!(),
        };

        let our_gov_version = gov.version;
        match our_gov_version.cmp(&gov_version) {
            std::cmp::Ordering::Less => {
                // Mi version es menor, me actualizo. y no le envío nada
                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver,
                    request_id: info.request_id,
                    reciver_actor: format!("/user/node/{}/distributor", gov_id),
                    schema: "governance".to_owned(),
                };

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(ActorError::NotHelper);
                };

                // TODO firmar la respuesta. Por ahora no, ya que si no tengo la gov en la última versión no puedo saber si es un testigo.
                if let Err(_e) = helper
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
                            gov_id
                        ),
                        schema: "governance".to_owned(),
                    };

                    let distributor_path = ActorPath::from(format!(
                        "/user/node/{}/distributor",
                        gov_id
                    ));
                    let distributor_actor: ActorRef<Distributor> =
                        if let Some(distributor_actor) =
                            ctx.system().get_actor(&distributor_path).await
                        {
                            distributor_actor
                        } else {
                            todo!()
                        };

                    if let Err(_e) = distributor_actor
                        .tell(DistributorMessage::SendDistribution {
                            gov_version: Some(gov_version),
                            actual_sn: Some(gov_version),
                            subject_id: gov_id.to_string(),
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

        Ok(CheckGovernance::Continue)
    }

    async fn check_gov_version(
        &self,
        ctx: &mut ActorContext<Distributor>,
        subject_id: &str,
        gov_version: Option<u64>,
        info: ComunicateInfo,
    ) -> Result<CheckGovernance, ActorError> {
        let gov = match get_gov(ctx, subject_id).await {
            Ok(gov) => gov,
            Err(_e) => todo!(),
        };

        let metadata = match get_metadata(ctx, subject_id).await {
            Ok(metadata) => metadata,
            Err(_e) => todo!(),
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
                        ctx.system().get_helper("network").await;

                    let Some(mut helper) = helper else {
                        ctx.system().send_event(SystemEvent::StopSystem).await;
                        return Err(ActorError::NotHelper);
                    };

                    // TODO firmar la respuesta. Por ahora no, ya que si no tengo la gov en la última versión no puedo saber si es un testigo.
                    if let Err(_e) = helper
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

                        if let Err(_e) = distributor_actor
                            .tell(DistributorMessage::SendDistribution {
                                gov_version: Some(gov_version),
                                actual_sn: Some(gov_version),
                                subject_id: metadata.governance_id.to_string(),
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
        } else if info.schema != "governance" {
            // Error me estás pidiendo el ledger para un sujeto que no es una governanza
            // Y no me pasas la versión de la gov
            todo!()
        }

        // Si el owner nos pide la copia.
        if metadata.owner == info.sender {
            return Ok(CheckGovernance::Continue);
        }

        if !gov.has_this_role(
            &info.sender.to_string(),
            Roles::WITNESS,
            &metadata.schema_id.clone(),
            metadata.namespace.clone(),
        ) {
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
    GetLastSn {
        subject_id: String,
        info: ComunicateInfo,
    },
    // Un nodo nos solicitó la copia del ledger.
    SendDistribution {
        gov_version: Option<u64>,
        actual_sn: Option<u64>,
        subject_id: String,
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
        _sender: ActorPath,
        msg: DistributorMessage,
        ctx: &mut ActorContext<Distributor>,
    ) -> Result<(), ActorError> {
        match msg {
            DistributorMessage::GetLastSn { subject_id, info } => {
                let Ok(metadata) = get_metadata(ctx, &subject_id).await else {
                    // Me está pidiendo info de un sujeto que no tengo
                    todo!()
                };

                let gov = match get_gov(ctx, &subject_id).await {
                    Ok(gov) => gov,
                    Err(_e) => todo!(),
                };

                if metadata.owner != info.sender
                    && !gov.has_this_role(
                        &info.sender.to_string(),
                        Roles::WITNESS,
                        &metadata.schema_id.clone(),
                        metadata.namespace.clone(),
                    )
                {
                    todo!()
                }

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    reciver_actor: format!(
                        "/user/node/auth/{}/{}",
                        subject_id, info.reciver
                    ),
                    schema: info.schema,
                };

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(ActorError::NotHelper);
                };

                // TODO firmar la respuesta.
                if let Err(_e) = helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::AuthLastSn {
                                sn: metadata.sn,
                            },
                        },
                    })
                    .await
                {
                    todo!()
                };
            }
            DistributorMessage::SendDistribution {
                actual_sn,
                mut info,
                gov_version,
                subject_id,
            } => {
                if info.schema.is_empty() {
                    let Ok(metadata) = get_metadata(ctx, &subject_id).await
                    else {
                        // Me está pidiendo info de un sujeto que no tengo
                        todo!()
                    };

                    let gov = match get_gov(ctx, &subject_id).await {
                        Ok(gov) => gov,
                        Err(_e) => todo!(),
                    };

                    if metadata.owner != info.sender
                        && !gov.has_this_role(
                            &info.sender.to_string(),
                            Roles::WITNESS,
                            &metadata.schema_id.clone(),
                            metadata.namespace.clone(),
                        )
                    {
                        todo!()
                    }

                    info.schema = metadata.schema_id;
                } else {
                    if let CheckGovernance::Finish = self
                        .check_gov_version(
                            ctx,
                            &subject_id,
                            gov_version,
                            info.clone(),
                        )
                        .await?
                    {
                        return Ok(());
                    };
                }

                let sn = if let Some(actual_sn) = actual_sn {
                    actual_sn
                } else {
                    0
                };

                // Sacar eventos.
                let (ledger, last_event) =
                    match self.get_ledger(ctx, &subject_id, sn).await {
                        Ok(res) => res,
                        Err(_e) => todo!(),
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
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(ActorError::NotHelper);
                };

                // TODO firmar la respuesta.
                if let Err(_e) = helper
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

                if let Err(_e) = retry.tell(RetryMessage::Retry).await {
                    todo!()
                };
            }
            DistributorMessage::NetworkResponse { signer } => {
                if signer == self.node {
                    let distribution_path = ctx.path().parent();

                    let distribution_actor: Option<ActorRef<Distribution>> =
                        ctx.system().get_actor(&distribution_path).await;

                    if let Some(distribution_actor) = distribution_actor {
                        if let Err(_e) = distribution_actor
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
                    if let Err(_e) = retry.tell(RetryMessage::End).await {
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
                if let Ok(CheckGovernance::Finish) =
                    Distributor::check_gov_version_ledger(
                        ctx,
                        info.clone(),
                        ledger.content.gov_version,
                        ledger.content.event_request.content.clone(),
                        ledger.content.subject_id.clone(),
                    )
                    .await
                {
                    return Ok(());
                };

                let new_subject = match self
                    .check_first_event(
                        ctx,
                        ledger.content.clone(),
                        info.reciver.clone(),
                        event.signature.signer.clone(),
                        &info.schema,
                    )
                    .await
                {
                    Ok(new) => new,
                    Err(_e) => todo!(),
                };
                // Llegados a este punto hemos verificado si es la primera copia o no,
                // Si es una gobernanza y está authorizada o el firmante del evento tiene el rol
                // de creator.

                // Ahora hay que crear el sujeto si no existe sn = 0, o aplicar los eventos
                // verificando los hashes y aplicando el patch.

                if new_subject {
                    // Creamos el sujeto.
                    if let Err(_e) = self.create_subject(ctx, ledger).await {
                        todo!()
                    };
                } else {
                    let subject_ref = match self
                        .get_subject(ctx, ledger.content.subject_id.clone())
                        .await
                    {
                        Ok(subject) => subject,
                        Err(_e) => todo!(),
                    };

                    let response = match subject_ref
                        .ask(SubjectMessage::UpdateLedger {
                            events: vec![ledger.clone()],
                        })
                        .await
                    {
                        Ok(res) => res,
                        Err(_e) => todo!(),
                    };

                    match response {
                        SubjectResponse::LastSn(last_sn) => {
                            // NO se aplicó el evento porque tendría un sn demasiado grande, no es el que toca o ya está aplicado.
                            // Si fue demasiado grande
                            if last_sn < ledger.content.sn {
                                let gov = match get_gov(
                                    ctx,
                                    &ledger.content.subject_id.to_string(),
                                )
                                .await
                                {
                                    Ok(gov) => gov,
                                    Err(_e) => todo!(),
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

                                let helper: Option<Intermediary> =
                                    ctx.system().get_helper("network").await;

                                let Some(mut helper) = helper else {
                                    ctx.system()
                                        .send_event(SystemEvent::StopSystem)
                                        .await;
                                    return Err(ActorError::NotHelper);
                                };

                                // Pedimos copia del ledger.
                                if let Err(_e) = helper.send_command(network::CommandHelper::SendMessage {
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

                if let Err(_e) = update_event(ctx, event.clone()).await {
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
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(ActorError::NotHelper);
                };

                if let Err(_e) = helper
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

                let subject_id = events[0].content.subject_id.clone();

                if let Ok(CheckGovernance::Finish) =
                    Distributor::check_gov_version_ledger(
                        ctx,
                        info.clone(),
                        events[0].content.gov_version,
                        events[0].content.event_request.content.clone(),
                        subject_id.clone(),
                    )
                    .await
                {
                    return Ok(());
                };

                let new_subject = match self
                    .check_first_event(
                        ctx,
                        events[0].content.clone(),
                        info.reciver.clone(),
                        events[0].signature.signer.clone(),
                        &info.schema,
                    )
                    .await
                {
                    Ok(new) => new,
                    Err(_e) => todo!(),
                };

                if new_subject {
                    // Creamos el sujeto.
                    if let Err(_e) =
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
                    Err(_e) => todo!(),
                };

                // Obtenemos el last_sn para saber si nos vale la pena intentar actualizar el ledger
                let response =
                    match subject_ref.ask(SubjectMessage::GetMetadata).await {
                        Ok(res) => res,
                        Err(_e) => todo!(),
                    };

                let metadata = match response {
                    SubjectResponse::Metadata(data) => data,
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
                        Err(_e) => todo!(),
                    };

                    match response {
                        SubjectResponse::LastSn(last_sn) => last_sn,
                        SubjectResponse::Error(e) => todo!(),
                        _ => todo!(),
                    }
                } else {
                    metadata.sn
                };

                if let Some(event) = last_event {
                    // si me envía esto quiere decir que ya no hay más eventos, terminó,
                    // Llegados a este punto este signed<Event> puede tener un sn inferior al mío.
                    // Entonces le tenemos que decir que se actualice.
                    // Si me actualicé ya no le digo nada más.
                    match last_sn.cmp(&last_sn_events) {
                        std::cmp::Ordering::Less => {
                            if let Err(_e) = update_event(ctx, event).await {
                                todo!()
                            };
                        }
                        std::cmp::Ordering::Equal => {}
                        std::cmp::Ordering::Greater => {
                            let our_path = ActorPath::from(format!(
                                "/user/node/{}/distributor",
                                subject_id
                            ));
                            let our_actor: Option<ActorRef<Distributor>> =
                                ctx.system().get_actor(&our_path).await;
                            if let Some(our_actor) = our_actor {
                                if let Err(_e) = our_actor
                                    .tell(
                                        DistributorMessage::SendDistribution {
                                            gov_version: Some(
                                                event.content.gov_version,
                                            ),
                                            actual_sn: Some(last_sn_events),
                                            subject_id: subject_id.to_string(),
                                            info,
                                        },
                                    )
                                    .await
                                {
                                    todo!()
                                }
                            } else {
                                todo!()
                            }
                        }
                    };

                    return Ok(());
                };

                let gov_id = if info.schema == "governance" {
                    metadata.subject_id
                } else {
                    metadata.governance_id
                };

                let gov = match get_gov(ctx, &gov_id.to_string()).await {
                    Ok(gov) => gov,
                    Err(_e) => todo!(),
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
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(ActorError::NotHelper);
                };

                if let Err(_e) = helper
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
        match error {
            ActorError::ReTry => {
                let distribuiton_path = ctx.path().parent();

                // Replication actor.
                let distribuiton_actor: Option<ActorRef<Distribution>> =
                    ctx.system().get_actor(&distribuiton_path).await;

                if let Some(distribuiton_actor) = distribuiton_actor {
                    if let Err(_e) = distribuiton_actor
                        .tell(DistributionMessage::Response {
                            sender: self.node.clone(),
                        })
                        .await
                    {
                        // TODO error, no se puede enviar la response
                        // return Err(_e);
                    }
                } else {
                    // TODO no se puede obtener evaluation! Parar.
                    // Can not obtain parent actor
                    // return Err(ActorError::Exists(evaluation_path));
                }
                ctx.stop().await;
            },
            _ => {
                // TODO error inesperado
            }
        };
    }
}
