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
use std::collections::{HashMap, VecDeque};
use store::store::PersistentActor;
use tracing::error;

use crate::{
    approval::approver::{ApprovalStateRes, Approver, ApproverMessage},
    db::Storable,
    governance::model::Roles,
    helpers::db::ExternalDB,
    init_state,
    model::common::{emit_fail, get_gov, get_metadata, get_quantity},
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
    ) -> Result<bool, ActorError> {
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<actor::ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        let response = if let Some(node_actor) = node_actor {
            node_actor
                .ask(NodeMessage::AmISubjectOwner(subject_id.to_owned()))
                .await?
        } else {
            return Err(ActorError::NotFound(node_path));
        };

        match response {
            NodeResponse::AmIOwner(owner) => Ok(owner),
            _ => Err(ActorError::UnexpectedResponse(node_path, "NodeResponse::AmIOwner".to_owned())),
        }
    }

    async fn queued_event(
        ctx: &mut ActorContext<RequestHandler>,
        subject_id: &str,
    ) -> Result<(), ActorError> {
        let request_path = ActorPath::from("/user/request");
        let request_actor: Option<actor::ActorRef<RequestHandler>> =
            ctx.system().get_actor(&request_path).await;

        if let Some(request_actor) = request_actor {
            request_actor
                .tell(RequestHandlerMessage::PopQueue {
                    subject_id: subject_id.to_owned(),
                })
                .await?
        } else {
            return Err(ActorError::NotFound(request_path));
        }
        Ok(())
    }

    async fn create_subject(
        ctx: &mut ActorContext<RequestHandler>,
        create_req: CreateRequest,
        request: Signed<EventRequest>,
    ) -> Result<DigestIdentifier, ActorError> {
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

        let subject_id = subject_id.hash_id(derivator).map_err(|e| ActorError::Functional(e.to_string()))?;

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
            let value = governance.get_init_state(&create_req.schema_id).map_err(|e| ActorError::Functional(e.to_string()))?;

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
            node_actor
                .ask(NodeMessage::CreateNewSubjectReq(data.clone()))
                .await?
        } else {
            return Err(ActorError::NotFound(node_path));
        };

        match response {
            NodeResponse::SonWasCreated => Ok(data.subject_id),
            _ => Err(ActorError::UnexpectedResponse(node_path, "NodeResponse::SonWasCreated".to_owned())),
        }
    }

    async fn error_queue_handling(
        &mut self,
        ctx: &mut ActorContext<RequestHandler>,
        id: &str,
        subject_id: &str,
    ) -> Result<(), ActorError> {
        self.on_event(
            RequestHandlerEvent::Invalid {
                id: id.to_owned(),
                subject_id: subject_id.to_owned(),
            },
            ctx,
        )
        .await;

        RequestHandler::queued_event(ctx, subject_id).await
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
    None
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
        self.init_store("request_handler", None, false, ctx).await?;
        
        let Some(ext_db): Option<ExternalDB> =
            ctx.system().get_helper("ext_db").await
        else {
            return Err(ActorError::NotHelper("ext_db".to_owned()));
        };

        for (subject_id, (request_id, request)) in self.handling.clone() {
            let request_manager =
                RequestManager::new(request_id.clone(), subject_id, request);
            let request_manager_actor = ctx.create_child(&request_id, request_manager).await?;
            let sink = Sink::new(
                request_manager_actor.subscribe(),
                ext_db.get_request_manager(),
            );
            ctx.system().run_sink(sink).await;

            request_manager_actor.tell(RequestManagerMessage::Run).await?;
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
                        return Err(ActorError::Functional(
                                "Invalid Response".to_owned(),
                            ),
                        )
                    }
                };

                let approver_path = ActorPath::from(format!(
                    "/user/node/{}/approver",
                    subject_id
                ));
                let approver_actor: Option<ActorRef<Approver>> = ctx
                    .system()
                    .get_actor(&approver_path)
                    .await;

                if let Some(approver_actor) = approver_actor {
                    if let Err(e) = approver_actor
                        .tell(ApproverMessage::ChangeResponse {
                            response: state.clone(),
                        })
                        .await
                    {
                        ctx.system().send_event(SystemEvent::StopSystem).await;
                        return Err(e);
                    }
                } else {
                    return Err(ActorError::NotFound(approver_path));
                };

                Ok(RequestHandlerResponse::Response(format!(
                    "The approval request for subject {} has changed to {}",
                    subject_id,
                    state.to_string()
                )))
            }
            RequestHandlerMessage::NewRequest { request } => {
                if let Err(e) = request.verify() {
                    return Err(ActorError::Functional(format!("Can not verify request signature {}", e)));
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
                            return Err(ActorError::Functional(
                                    "Only the node can sign creation events."
                                        .to_owned(),
                                ),
                            );
                        }

                        if create_request.schema_id == "governance" {
                            if !create_request.namespace.is_empty() {
                                return Err(ActorError::Functional("The creation event is for a governance, the namespace must be empty.".to_owned()));
                            }

                            if !create_request.governance_id.is_empty() {
                                return Err(ActorError::Functional("The creation event is for a governance, the governance_id must be empty.".to_owned()));
                            }
                        } else {
                            if create_request.governance_id.is_empty() {
                                return Err(ActorError::Functional("The creation event is for a traceability subject, the governance_id cannot be empty.".to_owned()));
                            }

                            let gov = match get_gov(ctx, &create_request.governance_id.to_string()).await {
                                Ok(gov) => gov,
                                Err(e) => return Err(ActorError::Functional(format!("It has not been possible to obtain governance: {}", e))),
                            };

                            if let Some(max_quantity) = gov.max_creations(
                                &self.node_key.to_string(),
                                &create_request.schema_id,
                                create_request.namespace.clone(),
                            ) {
                                let quantity = match get_quantity(ctx, create_request.governance_id.to_string(), create_request.schema_id.clone(), self.node_key.to_string(), create_request.namespace.to_string()).await {
                                    Ok(quantity) => quantity,
                                    Err(e) => return Err(ActorError::Functional(format!("An error occurred while processing the event: {}", e)))
                                };

                                if quantity >= max_quantity {
                                    return Err(ActorError::Functional(format!("The maximum number of subjects you can create for schema {} in governance {} has been reached.",create_request.schema_id, create_request.governance_id.to_string() )));
                                }
                            } else {
                                return Err(ActorError::Functional("The Scheme does not exist or does not have permissions for the creation of subjects, it needs to be assigned the creator role.".to_owned()));
                            };
                        }
                        let subject_id = match RequestHandler::create_subject(ctx, create_request, request.clone()).await {
                            Ok(subject_id) => subject_id,
                            Err(e) => return Err(ActorError::Functional(format!("An error has occurred and the subject could not be created: {}", e)))
                        };

                        let request_id = request.hash_id(derivator).map_err(|e| ActorError::Functional(format!("Can not obtain request hash id: {}", e)))?.to_string();

                        self.on_event(
                            RequestHandlerEvent::EventToQueue {
                                id: request_id.clone(),
                                subject_id: subject_id.to_string(),
                                event: request,
                            },
                            ctx,
                        )
                        .await;

                        if let Err(e) = RequestHandler::queued_event(
                            ctx,
                            &subject_id.to_string(),
                        )
                        .await
                        {
                            ctx.system().send_event(SystemEvent::StopSystem).await;
                            return Err(e);
                        }

                        return Ok(RequestHandlerResponse::Ok(RequestData {
                            request_id,
                            subject_id: subject_id.to_string(),
                        }));
                    }
                    EventRequest::Fact(fact_request) => fact_request.subject_id,
                    EventRequest::Transfer(transfer_request) => {
                        if request.signature.signer != self.node_key {
                            return Err(ActorError::Functional(
                                    "Only the node can sign creation events."
                                        .to_owned(),
                                ),
                            );
                        }
                        transfer_request.subject_id
                    }
                    EventRequest::Confirm(confirm_request) => {
                        if request.signature.signer != self.node_key {
                            return Err(ActorError::Functional(
                                    "Only the node can sign confirm events."
                                        .to_owned(),
                                ),
                            );
                        }
                        confirm_request.subject_id
                    }
                    EventRequest::EOL(eol_request) => {
                        if request.signature.signer != self.node_key {
                            return Err(ActorError::Functional(
                                    "Only the node can sign eol events."
                                        .to_owned(),
                                ),
                            );
                        }
                        eol_request.subject_id
                    }
                };

                if subject_id.is_empty() {
                    return Err(ActorError::Functional(
                            "Subject_id cannot be empty.".to_owned(),
                        ),
                    );
                }

                // Primero check que seamos el owner.
                let owner =
                    match Self::subject_owner(ctx, &subject_id.to_string())
                        .await
                    {
                        Ok(owner) => owner,
                        Err(e) => {
                            return Err(ActorError::Functional(format!(
                                    "An error has occurred: {}",
                                    e
                                )),
                            )
                        }
                    };

                if !owner {
                    if let EventRequest::Confirm(_confirm_request) =
                        request.content.clone()
                    {
                        // TODO VAMOS A Intentar actualizarnos, a lo mejor se ha hecho un evento de transferencia pero no lo hemos recibido.
                    } else {
                        return Err(ActorError::Functional("An event is being sent for a subject that does not belong to us.".to_owned()));
                    }
                }

                let metadata = get_metadata(ctx, &subject_id.to_string()).await?;

                if !metadata.active {
                    return Err(ActorError::Functional(
                            "The subject is no longer active.".to_owned(),
                        ),
                    );
                }

                let request_id = request.hash_id(derivator).map_err(|e| ActorError::Functional(format!("Can not obtain request id hash id: {}", e)))?.to_string();

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
                    if let Err(e) = RequestHandler::queued_event(
                        ctx,
                        &subject_id.to_string(),
                    )
                    .await
                    {
                        ctx.system().send_event(SystemEvent::StopSystem).await;
                        return Err(e);
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
                    // es imposible que no sea un option
                    return Ok(RequestHandlerResponse::None);
                };

                let request_id = match event.hash_id(derivator) {
                    Ok(request_id) => request_id.to_string(),
                    Err(e) => {
                        // YA previamente se ha generado el request id, por lo que no debería haber problema
                        let e = ActorError::Functional(format!("Can not obtain request id hash id: {}", e));
                        return Err(emit_fail(ctx, e).await);
                    },
                };

                let metadata =
                    match get_metadata(ctx, &subject_id.to_string()).await {
                        Ok(metadata) => metadata,
                        Err(e) => {
                            // YA previamente se ha obtenido la metadata, por lo que no debería haber problema
                            return Err(emit_fail(ctx, e).await);
                        },
                    };

                if metadata.owner != event.signature.signer {
                    if let Err(e) = self
                        .error_queue_handling(ctx, &request_id, &subject_id)
                        .await
                    {
                        return Err(emit_fail(ctx, e).await);
                    }

                    return Ok(RequestHandlerResponse::None);
                }

                if !metadata.active {
                    if let Err(e) =
                        self
                            .error_queue_handling(ctx, &request_id, &subject_id)
                            .await
                    {
                        return Err(emit_fail(ctx, e).await);
                    }

                    return Ok(RequestHandlerResponse::None);
                }

                let gov = match get_gov(ctx, &subject_id).await {
                    Ok(gov) => gov,
                    Err(e) => {
                        return Err(ActorError::Functional(format!(
                            "It has not been possible to obtain governance: {}",
                            e
                        )),
                        )
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
                                        if let Err(e) = self
                                            .error_queue_handling(
                                                ctx,
                                                &request_id,
                                                &subject_id,
                                            )
                                            .await
                                        {
                                            return Err(emit_fail(ctx, e).await);
                                        }

                                        return Ok(
                                            RequestHandlerResponse::None,
                                        );
                                    }
                                };

                                if quantity >= max_quantity {
                                    if let Err(e) = self
                                        .error_queue_handling(
                                            ctx,
                                            &request_id,
                                            &subject_id,
                                        )
                                        .await
                                    {
                                        return Err(emit_fail(ctx, e).await);
                                    }

                                    return Ok(RequestHandlerResponse::None);
                                }
                            } else {
                                if let Err(e) = self
                                    .error_queue_handling(
                                        ctx,
                                        &request_id,
                                        &subject_id,
                                    )
                                    .await
                                {
                                    return Err(emit_fail(ctx, e).await);
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
                            if let Err(e) = self
                                .error_queue_handling(
                                    ctx,
                                    &request_id,
                                    &subject_id,
                                )
                                .await
                            {
                                return Err(emit_fail(ctx, e).await);
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
                    Err(e) => return Err(emit_fail(ctx, e).await),
                };

                let Some(ext_db): Option<ExternalDB> =
                    ctx.system().get_helper("ext_db").await
                else {
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(ActorError::NotHelper("ext_db".to_owned()));
                };

                let sink = Sink::new(
                    request_actor.subscribe(),
                    ext_db.get_request_manager(),
                );
                ctx.system().run_sink(sink).await;

                if let Err(e) = request_actor.tell(message).await {
                    return Err(emit_fail(ctx, e).await);
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

                if let Err(e) =
                    RequestHandler::queued_event(ctx, &subject_id).await
                {
                    return Err(emit_fail(ctx, e).await);
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
            // TODO.
            println!("{}", e);
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
