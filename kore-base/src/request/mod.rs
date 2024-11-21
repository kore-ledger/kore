// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response, Sink, SystemEvent,
};
use async_trait::async_trait;
use identity::{
    identifier::{
        derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
    },
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use manager::{RequestManager, RequestManagerMessage};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    fmt::format,
};
use store::store::PersistentActor;
use tracing::error;

use crate::{
    approval::approver::{ApprovalStateRes, Approver, ApproverMessage},
    db::Storable,
    governance::model::Roles,
    helpers::db::ExternalDB,
    init_state,
    model::common::{get_gov, get_metadata, get_quantity},
    subject::{CreateSubjectData, SubjectID},
    CreateRequest, Error, EventRequest, HashId, Node, NodeMessage,
    NodeResponse, Signed, DIGEST_DERIVATOR,
};

pub mod manager;
pub mod state;

#[derive(Debug, Clone)]
pub struct RequestData {
    pub request_id: String,
    pub subject_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestHandler {
    node_key: KeyIdentifier,
    handling: HashMap<String, (String, Signed<EventRequest>)>,
    in_queue: HashMap<String, VecDeque<Signed<EventRequest>>>,
}

impl RequestHandler {
    pub fn new(node_key: KeyIdentifier) -> Self {
        RequestHandler {
            node_key,
            handling: HashMap::new(),
            in_queue: HashMap::new(),
        }
    }

    async fn subject_owner(
        ctx: &mut ActorContext<RequestHandler>,
        subject_id: &str,
    ) -> Result<bool, Error> {
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<actor::ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        let response = if let Some(node_actor) = node_actor {
            if let Ok(response) = node_actor
                .ask(NodeMessage::AmISubjectOwner(subject_id.to_owned()))
                .await
            {
                response
            } else {
                todo!()
            }
        } else {
            todo!()
        };

        match response {
            NodeResponse::AmIOwner(owner) => Ok(owner),
            _ => todo!(),
        }
    }

    async fn queued_event(
        ctx: &mut ActorContext<RequestHandler>,
        subject_id: &str,
    ) -> Result<(), Error> {
        let request_path = ActorPath::from("/user/request");
        let request_actor: Option<actor::ActorRef<RequestHandler>> =
            ctx.system().get_actor(&request_path).await;

        if let Some(request_actor) = request_actor {
            if let Err(_e) = request_actor
                .tell(RequestHandlerMessage::PopQueue {
                    subject_id: subject_id.to_owned(),
                })
                .await
            {
                todo!()
            }
        } else {
            todo!()
        }
        Ok(())
    }

    async fn create_subject(
        ctx: &mut ActorContext<RequestHandler>,
        create_req: CreateRequest,
        request: Signed<EventRequest>,
    ) -> Result<DigestIdentifier, Error> {
        let keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let subject_id = SubjectID {
            request: request.clone(),
            keys: keys.clone(),
        };

        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            DigestDerivator::Blake3_256
        };

        let subject_id = subject_id.hash_id(derivator)?;

        let data = if create_req.schema_id == "governance" {
            CreateSubjectData {
                keys,
                create_req,
                subject_id: subject_id,
                creator: request.signature.signer.clone(),
                genesis_gov_version: 0,
                value: init_state(&request.signature.signer.to_string()),
            }
        } else {
            let governance =
                get_gov(ctx, &create_req.governance_id.to_string()).await?;
            let value = governance.get_init_state(&create_req.schema_id)?;

            CreateSubjectData {
                keys,
                create_req,
                subject_id: subject_id,
                creator: request.signature.signer.clone(),
                genesis_gov_version: governance.version,
                value,
            }
        };

        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<actor::ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        let response = if let Some(node_actor) = node_actor {
            if let Ok(response) = node_actor
                .ask(NodeMessage::CreateNewSubjectReq(data.clone()))
                .await
            {
                response
            } else {
                todo!()
            }
        } else {
            todo!()
        };

        match response {
            NodeResponse::SonWasCreated => Ok(data.subject_id),
            NodeResponse::Error(e) => todo!(),
            _ => todo!(),
        }
    }

    async fn error_queue_handling(
        &mut self,
        ctx: &mut ActorContext<RequestHandler>,
        id: &str,
        subject_id: &str,
    ) -> Result<(), Error> {
        self.on_event(
            RequestHandlerEvent::Invalid {
                id: id.to_owned(),
                subject_id: subject_id.to_owned(),
            },
            ctx,
        )
        .await;

        if let Err(_e) = RequestHandler::queued_event(ctx, subject_id).await {
            todo!()
        }

        Ok(())
    }
}

// Enviar un evento sin firmar
// Enviar un evento firmado
// Aprobar

#[derive(Debug, Clone)]
pub enum RequestHandlerMessage {
    NewRequest {
        request: Signed<EventRequest>,
    },
    ChangeApprovalState {
        subject_id: String,
        state: ApprovalStateRes,
    },
    PopQueue {
        subject_id: String,
    },
    EndHandling {
        subject_id: String,
        id: String,
    },
}

impl Message for RequestHandlerMessage {}

#[derive(Debug, Clone)]
pub enum RequestHandlerResponse {
    Ok(RequestData),
    Response(String),
    Error(Error),
    None,
}

impl Response for RequestHandlerResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestHandlerEvent {
    EventToQueue {
        id: String,
        subject_id: String,
        event: Signed<EventRequest>,
    },

    Invalid {
        id: String,
        subject_id: String,
    },
    FinishHandling {
        id: String,
        subject_id: String,
    },
    EventToHandling {
        subject_id: String,
        request_id: String,
        event: Signed<EventRequest>,
    },
}

impl Event for RequestHandlerEvent {}

#[async_trait]
impl Actor for RequestHandler {
    type Event = RequestHandlerEvent;
    type Message = RequestHandlerMessage;
    type Response = RequestHandlerResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        // Cuando arranque tiene que levantar a todos los request que estén handling, si un request, está en starting, quiere
        // decir que todavía no inició nada, por lo tanto tiene que lazanle de nuevo el comando, de resto
        // Los request se manejan solo. TODO
        if let Err(_e) =
            self.init_store("request_handler", None, false, ctx).await
        {
            todo!()
        };
        let Some(ext_db): Option<ExternalDB> =
            ctx.system().get_helper("ext_db").await
        else {
            ctx.system().send_event(SystemEvent::StopSystem).await;
            return Err(ActorError::NotHelper);
        };

        for (subject_id, (request_id, request)) in self.handling.clone() {
            let request_manager =
                RequestManager::new(request_id.clone(), subject_id, request);
            let request_manager_actor =
                match ctx.create_child(&request_id, request_manager).await {
                    Ok(actor) => actor,
                    Err(e) => todo!(),
                };
            let sink = Sink::new(
                request_manager_actor.subscribe(),
                ext_db.get_request_manager(),
            );
            ctx.system().run_sink(sink).await;

            if let Err(e) =
                request_manager_actor.tell(RequestManagerMessage::Run).await
            {
                todo!()
            };
        }

        Ok(())
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<RequestHandler> for RequestHandler {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: RequestHandlerMessage,
        ctx: &mut actor::ActorContext<RequestHandler>,
    ) -> Result<RequestHandlerResponse, ActorError> {
        match msg {
            RequestHandlerMessage::ChangeApprovalState {
                subject_id,
                state,
            } => {
                match state.to_string().as_str() {
                    "RespondedAccepted" | "RespondedRejected" => {}
                    _ => {
                        return Ok(RequestHandlerResponse::Error(
                            Error::RequestHandler(
                                "Invalid Response".to_owned(),
                            ),
                        ))
                    }
                };

                let approver_actor: Option<ActorRef<Approver>> = ctx
                    .system()
                    .get_actor(&ActorPath::from(format!(
                        "/user/node/{}/approver",
                        subject_id
                    )))
                    .await;

                if let Some(approver_actor) = approver_actor {
                    if let Err(e) = approver_actor
                        .tell(ApproverMessage::ChangeResponse {
                            response: state.clone(),
                        })
                        .await
                    {
                        todo!()
                    }
                } else {
                    todo!()
                };

                Ok(RequestHandlerResponse::Response(format!(
                    "The approval request for subject {} has changed to {}",
                    subject_id,
                    state.to_string()
                )))
            }
            RequestHandlerMessage::NewRequest { request } => {
                if let Err(_e) = request.verify() {
                    todo!()
                };

                let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
                    *derivator
                } else {
                    error!("Error getting derivator");
                    DigestDerivator::Blake3_256
                };

                let subject_id = match request.content.clone() {
                    EventRequest::Create(create_request) => {
                        // verificar que el firmante sea el nodo.
                        if request.signature.signer != self.node_key {
                            return Ok(RequestHandlerResponse::Error(
                                Error::RequestHandler(
                                    "Only the node can sign creation events."
                                        .to_owned(),
                                ),
                            ));
                        }

                        if create_request.schema_id == "governance" {
                            if !create_request.namespace.is_empty() {
                                return Ok(RequestHandlerResponse::Error(Error::RequestHandler("The creation event is for a governance, the namespace must be empty.".to_owned())));
                            }

                            if !create_request.governance_id.is_empty() {
                                return Ok(RequestHandlerResponse::Error(Error::RequestHandler("The creation event is for a governance, the governance_id must be empty.".to_owned())));
                            }
                        } else {
                            if create_request.governance_id.is_empty() {
                                return Ok(RequestHandlerResponse::Error(Error::RequestHandler("The creation event is for a traceability subject, the governance_id cannot be empty.".to_owned())));
                            }

                            let gov = match get_gov(ctx, &create_request.governance_id.to_string()).await {
                                Ok(gov) => gov,
                                Err(e) => return Ok(RequestHandlerResponse::Error(Error::RequestHandler(format!("It has not been possible to obtain governance: {}", e)))),
                            };

                            if let Some(max_quantity) = gov.max_creations(
                                &self.node_key.to_string(),
                                &create_request.schema_id,
                                create_request.namespace.clone(),
                            ) {
                                let quantity = match get_quantity(ctx, create_request.governance_id.to_string(), create_request.schema_id.clone(), self.node_key.to_string(), create_request.namespace.to_string()).await {
                                    Ok(quantity) => quantity,
                                    Err(e) => return Ok(RequestHandlerResponse::Error(Error::RequestHandler(format!("An error occurred while processing the event: {}", e))))
                                };

                                if quantity >= max_quantity {
                                    return Ok(RequestHandlerResponse::Error(Error::RequestHandler(format!("The maximum number of subjects you can create for schema {} in governance {} has been reached.",create_request.schema_id, create_request.governance_id.to_string() ))));
                                }
                            } else {
                                return Ok(RequestHandlerResponse::Error(Error::RequestHandler("The Scheme does not exist or does not have permissions for the creation of subjects, it needs to be assigned the creator role.".to_owned())));
                            };
                        }
                        let subject_id = match RequestHandler::create_subject(ctx, create_request, request.clone()).await {
                            Ok(subject_id) => subject_id,
                            Err(e) => return Ok(RequestHandlerResponse::Error(Error::RequestHandler(format!("An error has occurred and the subject could not be created: {}", e))))
                        };

                        let request_id = match request.hash_id(derivator) {
                            Ok(request_id) => request_id.to_string(),
                            Err(_e) => todo!(),
                        };

                        self.on_event(
                            RequestHandlerEvent::EventToQueue {
                                id: request_id.clone(),
                                subject_id: subject_id.to_string(),
                                event: request,
                            },
                            ctx,
                        )
                        .await;

                        if let Err(_e) = RequestHandler::queued_event(
                            ctx,
                            &subject_id.to_string(),
                        )
                        .await
                        {
                            todo!()
                        }

                        return Ok(RequestHandlerResponse::Ok(RequestData {
                            request_id,
                            subject_id: subject_id.to_string(),
                        }));
                    }
                    EventRequest::Fact(fact_request) => fact_request.subject_id,
                    EventRequest::Transfer(transfer_request) => {
                        if request.signature.signer != self.node_key {
                            return Ok(RequestHandlerResponse::Error(
                                Error::RequestHandler(
                                    "Only the node can sign creation events."
                                        .to_owned(),
                                ),
                            ));
                        }
                        transfer_request.subject_id
                    }
                    EventRequest::Confirm(confirm_request) => {
                        if request.signature.signer != self.node_key {
                            return Ok(RequestHandlerResponse::Error(
                                Error::RequestHandler(
                                    "Only the node can sign creation events."
                                        .to_owned(),
                                ),
                            ));
                        }
                        confirm_request.subject_id
                    }
                    EventRequest::EOL(eol_request) => {
                        if request.signature.signer != self.node_key {
                            return Ok(RequestHandlerResponse::Error(
                                Error::RequestHandler(
                                    "Only the node can sign creation events."
                                        .to_owned(),
                                ),
                            ));
                        }
                        eol_request.subject_id
                    }
                };

                if subject_id.is_empty() {
                    return Ok(RequestHandlerResponse::Error(
                        Error::RequestHandler(
                            "Subject_id cannot be empty.".to_owned(),
                        ),
                    ));
                }

                // Primero check que seamos el owner.
                let owner =
                    match Self::subject_owner(ctx, &subject_id.to_string())
                        .await
                    {
                        Ok(owner) => owner,
                        Err(e) => {
                            return Ok(RequestHandlerResponse::Error(
                                Error::RequestHandler(format!(
                                    "An error has occurred: {}",
                                    e
                                )),
                            ))
                        }
                    };

                if !owner {
                    if let EventRequest::Confirm(_confirm_request) =
                        request.content.clone()
                    {
                        // TODO VAMOS A Intentar actualizarnos, a lo mejor se ha hecho un evento de transferencia pero no lo hemos recibido.
                    } else {
                        return Ok(RequestHandlerResponse::Error(Error::RequestHandler("An event is being sent for a subject that does not belong to us.".to_owned())));
                    }
                }

                let metadata =
                    match get_metadata(ctx, &subject_id.to_string()).await {
                        Ok(metadata) => metadata,
                        Err(_e) => todo!(),
                    };

                if !metadata.active {
                    return Ok(RequestHandlerResponse::Error(
                        Error::RequestHandler(
                            "The subject is no longer active.".to_owned(),
                        ),
                    ));
                }

                let request_id = match request.hash_id(derivator) {
                    Ok(request_id) => request_id.to_string(),
                    Err(_e) => todo!(),
                };

                self.on_event(
                    RequestHandlerEvent::EventToQueue {
                        id: request_id.clone(),
                        subject_id: subject_id.to_string(),
                        event: request,
                    },
                    ctx,
                )
                .await;

                if !self.handling.contains_key(&subject_id.to_string()) {
                    if let Err(_e) = RequestHandler::queued_event(
                        ctx,
                        &subject_id.to_string(),
                    )
                    .await
                    {
                        todo!()
                    }
                }

                Ok(RequestHandlerResponse::Ok(RequestData {
                    request_id,
                    subject_id: subject_id.to_string(),
                }))
            }
            RequestHandlerMessage::PopQueue { subject_id } => {
                // TODO, Ver si los que usan derivator nos renta que lo tengan directamente en memoria,
                // Para no estar pidiendolo a cada rato.
                let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
                    *derivator
                } else {
                    error!("Error getting derivator");
                    DigestDerivator::Blake3_256
                };

                let event = if let Some(events) = self.in_queue.get(&subject_id)
                {
                    if let Some(event) = events.clone().pop_front() {
                        event
                    } else {
                        // No hay más eventos pendientes.
                        return Ok(RequestHandlerResponse::None);
                    }
                } else {
                    // TODO es imposible que no sea un option
                    // TODO Cuando todo un protocolo falle y nadie responda, preguntarle al owner
                    // de la governanza si estamos actualizados, pueden haber cambiado todos los
                    // validadores o evaluadores, lo apunté aquí pero no va aquí
                    todo!()
                };

                let request_id = match event.hash_id(derivator) {
                    Ok(request_id) => request_id.to_string(),
                    Err(_e) => todo!(),
                };

                let metadata =
                    match get_metadata(ctx, &subject_id.to_string()).await {
                        Ok(metadata) => metadata,
                        Err(_e) => todo!(),
                    };

                if metadata.owner != event.signature.signer {
                    // TDO EVENTO DE FALLO
                    if let Err(_e) = self
                        .error_queue_handling(ctx, &request_id, &subject_id)
                        .await
                    {
                        todo!()
                    }

                    // TDO EVENTO DE FALLO
                    return Ok(RequestHandlerResponse::None);
                }

                if !metadata.active {
                    if let Err(_e) =
                        // TDO EVENTO DE FALLO
                        self
                            .error_queue_handling(ctx, &request_id, &subject_id)
                            .await
                    {
                        todo!()
                    }

                    return Ok(RequestHandlerResponse::None);
                }

                let gov = match get_gov(ctx, &subject_id).await {
                    Ok(gov) => gov,
                    Err(e) => {
                        return Ok(RequestHandlerResponse::Error(
                            Error::RequestHandler(format!(
                            "It has not been possible to obtain governance: {}",
                            e
                        )),
                        ))
                    }
                };

                let message = match event.content.clone() {
                    EventRequest::Create(create_request) => {
                        if create_request.schema_id != "governance" {
                            if let Some(max_quantity) = gov.max_creations(
                                &self.node_key.to_string(),
                                &create_request.schema_id,
                                create_request.namespace.clone(),
                            ) {
                                let quantity = match get_quantity(
                                    ctx,
                                    create_request.governance_id.to_string(),
                                    create_request.schema_id.clone(),
                                    self.node_key.to_string(),
                                    create_request.namespace.to_string(),
                                )
                                .await
                                {
                                    Ok(quantity) => quantity,
                                    Err(e) => {
                                        if let Err(_e) = self
                                            .error_queue_handling(
                                                ctx,
                                                &request_id,
                                                &subject_id,
                                            )
                                            .await
                                        {
                                            todo!()
                                        }

                                        return Ok(
                                            RequestHandlerResponse::None,
                                        );
                                    }
                                };

                                if quantity >= max_quantity {
                                    if let Err(_e) = self
                                        .error_queue_handling(
                                            ctx,
                                            &request_id,
                                            &subject_id,
                                        )
                                        .await
                                    {
                                        todo!()
                                    }

                                    return Ok(RequestHandlerResponse::None);
                                }
                            } else {
                                if let Err(_e) = self
                                    .error_queue_handling(
                                        ctx,
                                        &request_id,
                                        &subject_id,
                                    )
                                    .await
                                {
                                    todo!()
                                }

                                return Ok(RequestHandlerResponse::None);
                            };
                        }
                        RequestManagerMessage::Other
                    }
                    EventRequest::Fact(_fact_request) => {
                        let (signers, not_members) = gov.get_signers(
                            Roles::ISSUER,
                            &metadata.schema_id,
                            metadata.namespace.clone(),
                        );

                        if !signers
                            .iter()
                            .any(|x| x.clone() == event.signature.signer)
                            && (!not_members
                                || gov.members_to_key_identifier().iter().any(
                                    |x| x.clone() == event.signature.signer,
                                ))
                        {
                            // TDO EVENTO DE FALLO
                            if let Err(_e) = self
                                .error_queue_handling(
                                    ctx,
                                    &request_id,
                                    &subject_id,
                                )
                                .await
                            {
                                todo!()
                            }

                            return Ok(RequestHandlerResponse::None);
                        }
                        RequestManagerMessage::Fact
                    }
                    _ => RequestManagerMessage::Other,
                };

                let request_manager = RequestManager::new(
                    request_id.clone(),
                    subject_id.clone(),
                    event.clone(),
                );

                let request_actor = match ctx
                    .create_child(&request_id.clone(), request_manager)
                    .await
                {
                    Ok(request_actor) => request_actor,
                    Err(_e) => todo!(),
                };

                let Some(ext_db): Option<ExternalDB> =
                    ctx.system().get_helper("ext_db").await
                else {
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(ActorError::NotHelper);
                };

                let sink = Sink::new(
                    request_actor.subscribe(),
                    ext_db.get_request_manager(),
                );
                ctx.system().run_sink(sink).await;

                if let Err(_e) = request_actor.tell(message).await {
                    todo!()
                };

                self.on_event(
                    RequestHandlerEvent::EventToHandling {
                        subject_id: subject_id.clone(),
                        request_id: request_id,
                        event,
                    },
                    ctx,
                )
                .await;

                Ok(RequestHandlerResponse::None)
            }
            RequestHandlerMessage::EndHandling { subject_id, id } => {
                self.on_event(
                    RequestHandlerEvent::FinishHandling {
                        id,
                        subject_id: subject_id.clone(),
                    },
                    ctx,
                )
                .await;

                if let Err(_e) =
                    RequestHandler::queued_event(ctx, &subject_id).await
                {
                    todo!()
                }

                Ok(RequestHandlerResponse::None)
            }
        }
    }

    async fn on_event(
        &mut self,
        event: RequestHandlerEvent,
        ctx: &mut ActorContext<RequestHandler>,
    ) {
        if let Err(_e) = self.persist(&event, ctx).await {
            // TODO Propagar error.
        };

        if let Err(e) = ctx.publish_event(event).await {
            println!("{}", e);
            todo!()
        }
    }
}

#[async_trait]
impl Storable for RequestHandler {}

#[async_trait]
impl PersistentActor for RequestHandler {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) {
        match event {
            RequestHandlerEvent::EventToQueue {
                subject_id, event, ..
            } => {
                if let Some(vec) = self.in_queue.get_mut(subject_id) {
                    vec.push_back(event.clone());
                } else {
                    let mut vec = VecDeque::new();
                    vec.push_back(event.clone());
                    self.in_queue.insert(subject_id.clone(), vec);
                };
            }
            RequestHandlerEvent::Invalid { subject_id, .. } => {
                if let Some(vec) = self.in_queue.get_mut(subject_id) {
                    vec.pop_front();
                }
            }
            RequestHandlerEvent::EventToHandling {
                subject_id,
                request_id,
                event,
                ..
            } => {
                self.handling.insert(
                    subject_id.clone(),
                    (request_id.clone(), event.clone()),
                );
                if let Some(vec) = self.in_queue.get_mut(subject_id) {
                    vec.pop_front();
                }
            }
            RequestHandlerEvent::FinishHandling { subject_id, .. } => {
                self.handling.remove(subject_id);
            }
        }
    }
}
