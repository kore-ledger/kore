// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, ChildAction, Error as ActorError,
    Event, Handler, Message, Response, Sink, SystemEvent,
};
use async_trait::async_trait;
use identity::
    identifier::{
        derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
    };
use manager::{RequestManager, RequestManagerMessage};
use serde::{Deserialize, Serialize};
use types::ReqManInitMessage;
use std::collections::{HashMap, VecDeque};
use store::store::PersistentActor;
use tracing::{error, info};

use crate::{
    approval::approver::{ApprovalStateRes, Approver, ApproverMessage},
    db::Storable,
    governance::{model::CreatorQuantity, Governance},
    helpers::db::ExternalDB,
    init_state,
    model::{common::{get_gov, get_metadata, get_quantity, subject_owner}, Namespace},
    subject::CreateSubjectData,
    CreateRequest, EventRequest, HashId, Node, NodeMessage, NodeResponse,
    Signed, DIGEST_DERIVATOR,
};

pub mod manager;
pub mod reboot;
pub mod types;

const TARGET_REQUEST: &str = "Kore-Request";

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!(TARGET_REQUEST, "Error getting derivator");
            DigestDerivator::Blake3_256
        };

        let subject_id = request
            .hash_id(derivator)
            .map_err(|e| ActorError::Functional(e.to_string()))?;

        let data = if create_req.schema_id == "governance" {
            CreateSubjectData {
                create_req,
                subject_id,
                creator: request.signature.signer.clone(),
                genesis_gov_version: 0,
                value: init_state(&request.signature.signer.to_string()),
            }
        } else {
            let governance =
                get_gov(ctx, &create_req.governance_id.to_string()).await?;
            let value = governance
                .get_init_state(&create_req.schema_id)
                .map_err(|e| ActorError::Functional(e.to_string()))?;

            CreateSubjectData {
                create_req,
                subject_id,
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
            _ => Err(ActorError::UnexpectedResponse(
                node_path,
                "NodeResponse::SonWasCreated".to_owned(),
            )),
        }
    }

    async fn error_queue_handling(
        &mut self,
        ctx: &mut ActorContext<RequestHandler>,
        id: &str,
        subject_id: &str,
        error: &str,
    ) -> Result<(), ActorError> {
        self.on_event(
            RequestHandlerEvent::Invalid {
                id: id.to_owned(),
                subject_id: subject_id.to_owned(),
                error: error.to_owned(),
            },
            ctx,
        )
        .await;

        RequestHandler::queued_event(ctx, subject_id).await
    }

    async fn error(&mut self, ctx: &mut ActorContext<RequestHandler>, e: &str, subject_id: &str, request_id: &str) -> Result<RequestHandlerResponse, ActorError> {
        error!(
            TARGET_REQUEST,
            "PopQueue, {} for {}", e, subject_id
        );
        if let Err(e) = self
            .error_queue_handling(
                ctx,
                request_id,
                subject_id,
                e,
            )
            .await
        {
            error!(TARGET_REQUEST, "PopQueue, Can not enqueue next event: {}", e);
            ctx.system()
                .send_event(SystemEvent::StopSystem)
                .await;
            return Err(e);
        }

        Ok(RequestHandlerResponse::None)
    }

    pub async fn check_creations(&self, message: &str, ctx: &mut ActorContext<RequestHandler>, governance_id: &str, schema_id: &str, namespace: Namespace, gov: Governance) -> Result<(), ActorError> {
        if let Some(max_quantity) = gov.max_creations(
            &self.node_key.to_string(),
            schema_id,
            namespace.clone(),
        ) {
            if let CreatorQuantity::QUANTITY(max_quantity) =
                max_quantity
            {
                let quantity = match get_quantity(
                    ctx,
                    governance_id.to_string(),
                    schema_id.to_owned(),
                    self.node_key.to_string(),
                    namespace.to_string(),
                )
                .await
                {
                    Ok(quantity) => quantity,
                    Err(e) => {
                        error!(TARGET_REQUEST, "{}, can not get subject quatity of node: {}", message, e);
                        return Err(ActorError::Functional(format!("Can not get subject quatity of node: {}", e)));
                    }
                };

                if quantity >= max_quantity as usize {
                    error!(TARGET_REQUEST, "{}, The maximum number of subjects you can create for schema {} in governance {} has been reached.", message, schema_id, governance_id);
                    return Err(ActorError::Functional(format!("The maximum number of subjects you can create for schema {} in governance {} has been reached.", schema_id, governance_id)));
                }
            }

            Ok(())
        } else {
            let e = "The Scheme does not exist or does not have permissions for the creation of subjects, it needs to be assigned the creator role.";
            error!(TARGET_REQUEST, "{}, {}", message, e);
            
            Err(ActorError::Functional(
                e.to_owned(),
            ))
        }
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
    AbortRequest {
        subject_id: String,
        id: String,
        error: String,
    },
}

impl Message for RequestHandlerMessage {}

#[derive(Debug, Clone)]
pub enum RequestHandlerResponse {
    Ok(RequestData),
    Response(String),
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
        error: String,
    },
    Abort {
        id: String,
        subject_id: String,
        error: String,
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
        self.init_store("request", None, false, ctx).await?;

        let Some(ext_db): Option<ExternalDB> =
            ctx.system().get_helper("ext_db").await
        else {
            return Err(ActorError::NotHelper("ext_db".to_owned()));
        };

        for (subject_id, (request_id, request)) in self.handling.clone() {
            let request_manager = RequestManager::new(
                self.node_key.clone(),
                request_id.clone(),
                subject_id,
                request,
                ReqManInitMessage::Validate
            );
            let request_manager_actor =
                ctx.create_child(&request_id, request_manager).await?;
            let sink = Sink::new(
                request_manager_actor.subscribe(),
                ext_db.get_request_manager(),
            );
            ctx.system().run_sink(sink).await;

            request_manager_actor
                .tell(RequestManagerMessage::Run)
                .await?;
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
            RequestHandlerMessage::AbortRequest {
                subject_id,
                id,
                error,
            } => {
                info!(
                    TARGET_REQUEST,
                    "AbortRequest, Aborting request {} for {}: {}",
                    id,
                    subject_id,
                    error
                );

                self.on_event(
                    RequestHandlerEvent::Abort {
                        id: id.to_owned(),
                        subject_id: subject_id.to_owned(),
                        error: error.to_owned(),
                    },
                    ctx,
                )
                .await;

                Ok(RequestHandlerResponse::None)
            }
            RequestHandlerMessage::ChangeApprovalState {
                subject_id,
                state,
            } => {
                info!(TARGET_REQUEST, "New approval response");

                match state.to_string().as_str() {
                    "RespondedAccepted" | "RespondedRejected" => {}
                    _ => {
                        error!(
                            TARGET_REQUEST,
                            "ChangeApprovalState, Invalid approval response"
                        );
                        return Err(ActorError::Functional(
                            "Invalid Response".to_owned(),
                        ));
                    }
                };

                let approver_path = ActorPath::from(format!(
                    "/user/node/{}/approver",
                    subject_id
                ));
                let approver_actor: Option<ActorRef<Approver>> =
                    ctx.system().get_actor(&approver_path).await;

                if let Some(approver_actor) = approver_actor {
                    if let Err(e) = approver_actor
                        .tell(ApproverMessage::ChangeResponse {
                            response: state.clone(),
                        })
                        .await
                    {
                        error!(TARGET_REQUEST, "ChangeApprovalState, can not send message to Approver actor: {}", e);
                        ctx.system().send_event(SystemEvent::StopSystem).await;
                        return Err(e);
                    }
                } else {
                    error!(
                        TARGET_REQUEST,
                        "ChangeApprovalState, can not obtain Approver actor"
                    );
                    return Err(ActorError::NotFound(approver_path));
                };

                Ok(RequestHandlerResponse::Response(format!(
                    "The approval request for subject {} has changed to {}",
                    subject_id, state
                )))
            }
            RequestHandlerMessage::NewRequest { request } => {
                if let Err(e) = request.verify() {
                    error!(
                        TARGET_REQUEST,
                        "NewRequest, can not verify new request: {}", e
                    );
                    return Err(ActorError::Functional(format!(
                        "Can not verify request signature {}",
                        e
                    )));
                };

                let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
                    *derivator
                } else {
                    error!(TARGET_REQUEST, "Error getting derivator");
                    DigestDerivator::Blake3_256
                };

                let metadata = match request.content.clone() {
                    EventRequest::Create(create_request) => {
                        // verificar que el firmante sea el nodo.
                        if request.signature.signer != self.node_key {
                            let e = "Only the node can sign creation events.";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }

                        if create_request.schema_id == "governance" {
                            if !create_request.namespace.is_empty() {
                                let e = "The creation event is for a governance, the namespace must be empty.";
                                error!(TARGET_REQUEST, "NewRequest, {}", e);
                                return Err(ActorError::Functional(
                                    e.to_owned(),
                                ));
                            }

                            if !create_request.governance_id.is_empty() {
                                let e = "The creation event is for a governance, the governance_id must be empty.";
                                error!(TARGET_REQUEST, "NewRequest, {}", e);
                                return Err(ActorError::Functional(
                                    e.to_owned(),
                                ));
                            }
                        } else {
                            if create_request.governance_id.is_empty() {
                                let e = "The creation event is for a traceability subject, the governance_id cannot be empty.";
                                error!(TARGET_REQUEST, "NewRequest, {}", e);
                                return Err(ActorError::Functional(
                                    e.to_owned(),
                                ));
                            }

                            let gov = match get_gov(
                                ctx,
                                &create_request.governance_id.to_string(),
                            )
                            .await
                            {
                                Ok(gov) => gov,
                                Err(e) => {
                                    error!(TARGET_REQUEST, "NewRequest, can not get governance: {}", e);
                                    return Err(ActorError::Functional(format!("It has not been possible to obtain governance: {}", e)));
                                }
                            };

                            self.check_creations("NewRequest", ctx, &create_request.governance_id.to_string(), &create_request.schema_id, create_request.namespace.clone(), gov).await?;
                        }
                        let subject_id = match RequestHandler::create_subject(
                            ctx,
                            create_request,
                            request.clone(),
                        )
                        .await
                        {
                            Ok(subject_id) => subject_id,
                            Err(e) => {
                                error!(TARGET_REQUEST, "NewRequest, An error has occurred and the subject could not be created: {}", e);
                                return Err(ActorError::Functional(format!("An error has occurred and the subject could not be created: {}", e)));
                            }
                        };

                        let request_id = request
                            .hash_id(derivator)
                            .map_err(|e| {
                                error!(TARGET_REQUEST, "NewRequest, Can not obtain request hash id: {}", e);
                                ActorError::Functional(format!(
                                    "Can not obtain request hash id: {}",
                                    e
                                ))
                            })?
                            .to_string();

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
                            error!(
                                TARGET_REQUEST,
                                "NewRequest, Can not enqueue new event: {}", e
                            );
                            ctx.system()
                                .send_event(SystemEvent::StopSystem)
                                .await;
                            return Err(e);
                        }

                        return Ok(RequestHandlerResponse::Ok(RequestData {
                            request_id,
                            subject_id: subject_id.to_string(),
                        }));
                    }
                    EventRequest::Fact(fact_request) => {
                        let metadata = get_metadata(ctx, &fact_request.subject_id.to_string()).await?;

                        if metadata.new_owner.is_some() {
                            let e = "After Transfer event only can emit Confirm or Reject event";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }

                        metadata
                    }
                    EventRequest::Transfer(transfer_request) => {
                        if request.signature.signer != self.node_key {
                            let e = "Only the node can sign transfer events.";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }

                        let metadata = get_metadata(ctx, &transfer_request.subject_id.to_string()).await?;

                        if metadata.new_owner.is_some() {
                            let e = "After Transfer event only can emit Confirm or Reject event";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }

                        metadata
                    }
                    EventRequest::Confirm(confirm_request) => {
                        if request.signature.signer != self.node_key {
                            let e = "Only the node can sign Confirm events.";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }
                        let metadata =
                            get_metadata(ctx, &confirm_request.subject_id.to_string()).await?;

                        let Some(new_owner) = metadata.new_owner.clone() else {
                            let e = "Confirm event need Transfer event before";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        };

                        if new_owner != self.node_key {
                            let e = "You are not new owner";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }

                        if !metadata.governance_id.is_empty() {

                            let gov = match get_gov(
                                ctx,
                                &metadata.governance_id.to_string(),
                            )
                            .await
                            {
                                Ok(gov) => gov,
                                Err(e) => {
                                    error!(TARGET_REQUEST, "NewRequest, can not get governance: {}", e);
                                    return Err(ActorError::Functional(format!("It has not been possible to obtain governance: {}", e)));
                                }
                            };

                            self.check_creations("NewRequest", ctx, &metadata.governance_id.to_string(), &metadata.schema_id, metadata.namespace.clone(), gov).await?;
                        }

                        metadata
                    }
                    EventRequest::Reject(reject_request) => {
                        if request.signature.signer != self.node_key {
                            let e = "Only the node can sign reject events.";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }
                        let metadata =
                        get_metadata(ctx, &reject_request.subject_id.to_string()).await?;

                        let Some(new_owner) = metadata.new_owner.clone() else {
                            let e = "Confirm event need Transfer event before";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        };

                        if new_owner != self.node_key {
                            let e = "You are not new owner";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }

                        metadata
                    },
                    EventRequest::EOL(eol_request) => {
                        if request.signature.signer != self.node_key {
                            let e = "Only the node can sign eol events.";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }

                        let metadata = get_metadata(ctx, &eol_request.subject_id.to_string()).await?;

                        if metadata.new_owner.is_some() {
                            let e = "After Transfer event only can emit Confirm or Reject event";
                            error!(TARGET_REQUEST, "NewRequest, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        }

                        metadata
                    },
                };

                // TODO MIRAR QUE NO ESTÉ COMO TEMPORAL EL SUBJECT.

                // Primero check que seamos el owner.
                let (is_owner, is_pending) = subject_owner(
                    ctx,
                    &metadata.subject_id.to_string(),
                )
                .await.map_err(|e| {
                    error!(TARGET_REQUEST, "NewRequest, Could not determine if the node is the owner of the subject: {}", e);
                    ActorError::Functional(format!(
                        "An error has occurred: {}",
                        e
                    ))
                })?;

                if !is_owner && !is_pending {
                    let e =  "An event is being sent to a subject that does not belong to us or its creation is pending completion, and the subject is not pending event confirmation.";
                    error!(
                        TARGET_REQUEST,
                        "NewRequest, {}", e
                    );
                    return Err(ActorError::Functional(e.to_owned()));
                }

                if is_owner && is_pending {
                    let e =  "We are the owner of the subject but this subject is pending transfer";
                    error!(
                        TARGET_REQUEST,
                        "NewRequest, {}", e
                    );
                    return Err(ActorError::Functional(e.to_owned()));
                }

                if !metadata.active {
                    let e = "The subject is no longer active.";
                    error!(TARGET_REQUEST, "NewRequest, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                }

                let request_id = request
                    .hash_id(derivator)
                    .map_err(|e| {
                        error!(
                            TARGET_REQUEST,
                            "NewRequest, Can not obtain request id hash id: {}",
                            e
                        );
                        ActorError::Functional(format!(
                            "Can not obtain request id hash id: {}",
                            e
                        ))
                    })?
                    .to_string();

                self.on_event(
                    RequestHandlerEvent::EventToQueue {
                        id: request_id.clone(),
                        subject_id: metadata.subject_id.to_string(),
                        event: request,
                    },
                    ctx,
                )
                .await;

                if !self.handling.contains_key(&metadata.subject_id.to_string()) {
                    if let Err(e) = RequestHandler::queued_event(
                        ctx,
                        &metadata.subject_id.to_string(),
                    )
                    .await
                    {
                        error!(
                            TARGET_REQUEST,
                            "NewRequest, Can not enqueue new event: {}", e
                        );
                        ctx.system().send_event(SystemEvent::StopSystem).await;
                        return Err(e);
                    }
                }

                Ok(RequestHandlerResponse::Ok(RequestData {
                    request_id,
                    subject_id: metadata.subject_id.to_string(),
                }))
            }
            RequestHandlerMessage::PopQueue { subject_id } => {
                let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
                    *derivator
                } else {
                    error!(TARGET_REQUEST, "Error getting derivator");
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
                        error!(
                            TARGET_REQUEST,
                            "PopQueue, Can not obtain request id hash id: {}",
                            e
                        );
                        let e = ActorError::Functional(format!(
                            "Can not obtain request id hash id: {}",
                            e
                        ));
                        ctx.system().send_event(SystemEvent::StopSystem).await;
                        return Err(e);
                    }
                };

                let metadata = match get_metadata(ctx, &subject_id.to_string())
                    .await
                {
                    Ok(metadata) => metadata,
                    Err(e) => {
                        // YA previamente se ha obtenido la metadata, por lo que no debería haber problema
                        error!(
                            TARGET_REQUEST,
                            "PopQueue, Can not obtain subject metadata: {}", e
                        );
                        ctx.system().send_event(SystemEvent::StopSystem).await;
                        return Err(e);
                    }
                };

                if !metadata.active {
                    let e = "Subject is not active";
                    return self.error(ctx, e, &subject_id, &request_id).await
                }

                let gov = match get_gov(ctx, &subject_id).await {
                    Ok(gov) => gov,
                    Err(e) => {
                        error!(
                            TARGET_REQUEST,
                            "PopQueue, Can not get governance: {}", e
                        );
                        return Err(ActorError::Functional(format!(
                            "It has not been possible to obtain governance: {}",
                            e
                        )));
                    }
                };

                if !event.content.check_signers(&event.signature.signer, &metadata, &gov) {
                    let e = "Invalid signer for this event";
                    return self.error(ctx, e, &subject_id, &request_id).await
                }

                let (message, command) = match event.content.clone() {
                    EventRequest::Create(create_request) => {
                        if create_request.schema_id != "governance" {

                            if let Err(e) = self.check_creations("PopQueue", ctx, &metadata.governance_id.to_string(), &metadata.schema_id, metadata.namespace.clone(), gov).await {
                                return self.error(ctx, &e.to_string(), &subject_id, &request_id).await
                            };
                        }
                        (RequestManagerMessage::Validate, ReqManInitMessage::Validate)
                    }
                    
                    EventRequest::Confirm(confirm_req) => {
                        if metadata.governance_id.is_empty() {
                            if let Some(name) = confirm_req.name_old_owner {
                                if name.is_empty() {
                                    let e = "Name of old owner can not be a empty String";
                                    return self.error(ctx, e, &subject_id, &request_id).await
                                }
                            }
                            (RequestManagerMessage::Evaluate, ReqManInitMessage::Evaluate)
                        } else {
                            if confirm_req.name_old_owner.is_some() {
                                let e = "Name of old owner must be None";
                                return self.error(ctx, e, &subject_id, &request_id).await
                            }

                            if let Err(e) = self.check_creations("PopQueue", ctx, &metadata.governance_id.to_string(), &metadata.schema_id, metadata.namespace.clone(), gov).await {
                                return self.error(ctx, &e.to_string(), &subject_id, &request_id).await
                            };
                            (RequestManagerMessage::Validate, ReqManInitMessage::Validate)
                        }
                    },
                    EventRequest::Fact(_) |
                    EventRequest::Transfer(_)  => {
                        (RequestManagerMessage::Evaluate, ReqManInitMessage::Evaluate)
                    },
                    _ => (RequestManagerMessage::Validate, ReqManInitMessage::Validate),
                };

                let request_manager = RequestManager::new(
                    self.node_key.clone(),
                    request_id.clone(),
                    subject_id.clone(),
                    event.clone(),
                    command
                );

                let request_actor = match ctx
                    .create_child(&request_id.clone(), request_manager)
                    .await
                {
                    Ok(request_actor) => request_actor,
                    Err(e) => {
                        error!(TARGET_REQUEST, "PopQueue, Can not create request manager actor: {}", e);
                        ctx.system().send_event(SystemEvent::StopSystem).await;
                        return Err(e);
                    }
                };

                let Some(ext_db): Option<ExternalDB> =
                    ctx.system().get_helper("ext_db").await
                else {
                    error!(
                        TARGET_REQUEST,
                        "PopQueue, Can not obtaint ext_db helper"
                    );
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(ActorError::NotHelper("ext_db".to_owned()));
                };

                let sink = Sink::new(
                    request_actor.subscribe(),
                    ext_db.get_request_manager(),
                );
                ctx.system().run_sink(sink).await;

                if let Err(e) = request_actor.tell(message).await {
                    error!(TARGET_REQUEST, "PopQueue, Can not send message to request manager actor: {}", e);
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(e);
                };

                self.on_event(
                    RequestHandlerEvent::EventToHandling {
                        subject_id: subject_id.clone(),
                        request_id,
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
                    error!(
                        TARGET_REQUEST,
                        "EndHandling, Can not enqueue next event: {}", e
                    );
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(e);
                }

                Ok(RequestHandlerResponse::None)
            }
        }
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<RequestHandler>,
    ) -> ChildAction {
        error!(TARGET_REQUEST, "OnChildFault, {}", error);
        ctx.system().send_event(SystemEvent::StopSystem).await;
        ChildAction::Stop
    }

    async fn on_event(
        &mut self,
        event: RequestHandlerEvent,
        ctx: &mut ActorContext<RequestHandler>,
    ) {
        if let Err(e) = self.persist_light(&event, ctx).await {
            error!(
                TARGET_REQUEST,
                "OnEvent, can not persist information: {}", e
            );
            ctx.system().send_event(SystemEvent::StopSystem).await;
        };

        if let Err(e) = ctx.publish_event(event).await {
            error!(
                TARGET_REQUEST,
                "PublishEvent, can not publish event: {}", e
            );
            ctx.system().send_event(SystemEvent::StopSystem).await;
        }
    }
}

#[async_trait]
impl Storable for RequestHandler {}

#[async_trait]
impl PersistentActor for RequestHandler {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        match event {
            RequestHandlerEvent::Abort { subject_id, .. } => {
                self.handling.remove(subject_id);
            }
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
        };

        Ok(())
    }
}
