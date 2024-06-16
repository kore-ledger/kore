// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::req::Request;

use crate::{
    db::Database,
    model::{request::EventRequest, signature::Signed},
    Error,
};

use actor::{
    Actor, ActorContext, Error as ActorError, Event, Handler, Message, Response,
};
use identity::identifier::DigestIdentifier;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};

use std::collections::HashSet;

/// Request handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestHandler {
    requests: HashSet<DigestIdentifier>,
}

impl RequestHandler {
    /// Creates a new `RequestHandler`.
    pub fn new() -> Self {
        RequestHandler::default()
    }

    /// Does the request exist?
    pub fn request_exists(&self, request_id: &DigestIdentifier) -> bool {
        self.requests.contains(request_id)
    }

    /// Handles the `StartRequest` message.
    /// This method is called when a `StartRequest` message is received.
    async fn handle_start_request(
        &mut self,
        msg: Signed<EventRequest>,
        ctx: &mut ActorContext<RequestHandler>,
    ) -> RequestResponse {
        // Verify the request signature.
        if msg.verify().is_err() {
            error!("invalid signature for request");
            return RequestResponse::Error(Error::RequestEvent(
                "invalid signature".to_owned(),
            ));
        }
        // Generates request identifier.
        let request_id = match DigestIdentifier::generate_with_blake3(&msg) {
            Ok(id) => id,
            Err(e) => {
                error!("failed to generate request identifier: {}", e);
                return RequestResponse::Error(Error::RequestEvent(
                    e.to_string(),
                ));
            }
        };

        // Does the request exist?
        if self.request_exists(&request_id) {
            error!("request already exists: {}", request_id);
            return RequestResponse::Error(Error::RequestEvent(
                "request already exists".to_owned(),
            ));
        }

        // Emits the `RequestEvent` event.
        if ctx
            .event(RequestHandlerEvent::Start {
                id: request_id.clone(),
                request: msg,
            })
            .await
            .is_err()
        {
            error!("failed to emit event");
            return RequestResponse::Error(Error::RequestEvent(
                "failed to emit event".to_owned(),
            ));
        }

        RequestResponse::CreateRequest(request_id)
    }

    /// Handles the `EndRequest` message.
    /// This method is called when a `EndRequest` message is received.
    async fn handle_end_request(
        &mut self,
        id: DigestIdentifier,
        ctx: &mut ActorContext<RequestHandler>,
    ) -> RequestResponse {
        // Does the request exist?
        if !self.request_exists(&id) {
            error!("request does not exist: {}", id);
            return RequestResponse::Error(Error::RequestEvent(
                "request does not exist".to_owned(),
            ));
        }

        // Emits the `RequestEvent` event.
        if ctx.event(RequestHandlerEvent::End { id }).await.is_err() {
            error!("failed to emit event");
            return RequestResponse::Error(Error::RequestEvent(
                "failed to emit event".to_owned(),
            ));
        }

        RequestResponse::None
    }
}

impl Default for RequestHandler {
    fn default() -> Self {
        RequestHandler {
            requests: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum RequestHandlerCommand {
    StartRequest(Signed<EventRequest>),
    EndRequest(DigestIdentifier),
}

/// Message implementation for `RequestHandlerCommand`.
impl Message for RequestHandlerCommand {}

/// Request responser.
#[derive(Debug, Clone)]
pub enum RequestResponse {
    CreateRequest(DigestIdentifier),
    Error(Error),
    None,
}

/// Response implementation for `EventResponse`.
impl Response for RequestResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestHandlerEvent {
    Start {
        id: DigestIdentifier,
        request: Signed<EventRequest>,
    },
    End {
        id: DigestIdentifier,
    },
}

/// Event implementation for `RequestEvent`.
impl Event for RequestHandlerEvent {}

/// Actor implementation for `RequestHandler`.
/// The `RequestHandler` actor is responsible for handling requests.
/// It receives `EventRequest` events and responds with a `RequestResponse` and emits
/// `RequestEvent`.
#[async_trait]
impl Actor for RequestHandler {
    type Event = RequestHandlerEvent;
    type Message = RequestHandlerCommand;
    type Response = RequestResponse;

    /// Pre-start implementation for `RequestHandler`.
    async fn pre_start(
        &mut self,
        ctx: &mut actor::ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Creating RequestHandler store");
        // Gets database
        let db = match ctx.system().get_helper::<Database>("db").await {
            Some(db) => db,
            None => {
                error!("Database not found");
                return Err(ActorError::CreateStore(
                    "Database not found".to_string(),
                ));
            }
        };
        // Start store
        self.start_store(ctx, db, None).await?;
        Ok(())
    }

    /// Post-stop implementation for `RequestHandler`.
    async fn post_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await?;
        Ok(())
    }
}

/// Handler implementation for `RequestHandler`.
///
#[async_trait]
impl Handler<RequestHandler> for RequestHandler {
    /// Handles the `EventRequest` message.
    async fn handle_message(
        &mut self,
        msg: RequestHandlerCommand,
        ctx: &mut ActorContext<RequestHandler>,
    ) -> RequestResponse {
        match msg {
            RequestHandlerCommand::StartRequest(msg) => {
                self.handle_start_request(msg, ctx).await
            }
            RequestHandlerCommand::EndRequest(id) => {
                self.handle_end_request(id, ctx).await
            }
        }
    }

    /// Handles the `EventRequest` event.
    async fn handle_event(
        &mut self,
        event: RequestHandlerEvent,
        ctx: &mut ActorContext<RequestHandler>,
    ) {
        match event.clone() {
            RequestHandlerEvent::Start { id, request } => {
                // Create child request
                if ctx
                    .create_child(&id.to_string(), Request::new(id, request))
                    .await
                    .is_err()
                {
                    let _ = ctx
                        .emit_fail(ActorError::CreateStore(
                            "Failed to create child request".to_string(),
                        ))
                        .await;
                    return;
                }
                if self.persist(event.clone(), ctx).await.is_err() {
                    let _ = ctx
                        .emit_fail(ActorError::CreateStore(
                            "Failed to persist event".to_string(),
                        ))
                        .await;
                    return;
                }
            }
            RequestHandlerEvent::End { id } => {
                // Stop child.
                let child =
                    match ctx.get_child::<Request>(&id.to_string()).await {
                        Some(child) => child,
                        None => {
                            let _ =
                                ctx.emit_fail(ActorError::CreateStore(
                                    format!("Failed to get child {}", id),
                                ))
                                .await;
                            return;
                        }
                    };
                child.stop().await;
                if self.persist(event.clone(), ctx).await.is_err() {
                    let _ = ctx
                        .emit_fail(ActorError::CreateStore(
                            "Failed to persist event".to_string(),
                        ))
                        .await;
                    return;
                }
            }
        }
        self.apply(event);
    }
}

#[async_trait]
impl PersistentActor for RequestHandler {
    /// Change request handler state.
    fn apply(&mut self, event: Self::Event) {
        match event {
            RequestHandlerEvent::Start { id, .. } => {
                self.requests.insert(id);
            }
            RequestHandlerEvent::End { id } => {
                self.requests.remove(&id);
            }
        }
    }
}
