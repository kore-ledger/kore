// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    db::Database, governance::{self, Governance}, model::{request::EventRequest, signature::Signed}, Error
};

use actor::{
    Actor, ActorContext, Error as ActorError, Event, Handler, Message, Response, ActorPath,
};
use identity::identifier::{DigestIdentifier, KeyIdentifier};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error, field::debug};

use std::collections::HashSet;

/// Request handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestHandler {
    /// Owner identifier.
    id: KeyIdentifier,
    requests: HashSet<DigestIdentifier>,
}

impl RequestHandler {
    /// Creates a new `RequestHandler`.
    pub fn new(id: KeyIdentifier) -> Self {
        debug!("Create Request handler for: {}", id);
        Self {
            id,
            requests: HashSet::new(),
        }
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
        // Check if the signer is the owner of the node for creation events.
        if let EventRequest::Create(start) = &msg.content {
            if msg.signature.signer != self.id {
                error!("invalid signer for request");
                return RequestResponse::Error(Error::RequestEvent(
                    "invalid signer".to_owned(),
                ));
            }
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
        if self.requests.contains(&request_id) {
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
        if !self.requests.contains(&id) {
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
                debug!("Handle start request");
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
                match &request.content {
                    EventRequest::Create(request) => {
                        debug!("Create request actor");


                    }
                    _ => {}
                }

            },
            RequestHandlerEvent::End { id } => {

            },
        }
        if let Err(error) = self.persist(&event, ctx).await {
            if ctx.emit_fail(error).await.is_err(){
                error!("Emit fail");
            }
        }
        self.apply(&event);
    }
}

#[async_trait]
impl PersistentActor for RequestHandler {
    /// Change request handler state.
    fn apply(&mut self, event: &Self::Event) {
        match event {
            RequestHandlerEvent::Start { id, .. } => {
                self.requests.insert(id.clone());
            }
            RequestHandlerEvent::End { id } => {
                self.requests.remove(&id);
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use identity::{
        identifier::derive::digest::DigestDerivator,
        keys::{Ed25519KeyPair, KeyPair, KeyGenerator, KeyMaterial}
    };

    use crate::{model::{request::EventRequest, signature::Signature}, system, Config, DbConfig};

    use super::*;

    use tracing_test::traced_test;
    use serde_json::{self, json};

    #[tokio::test]
    #[traced_test]
    async fn test_request_handler() {
        let kp = KeyPair::Ed25519(Ed25519KeyPair::new());
        let dir =
            tempfile::tempdir().expect("Can not create temporal directory.");
        let path = dir.path().to_str().unwrap().to_owned(); 
        let db = DbConfig::Rocksdb { path };
        let config = Config { database: db};
        let system = system(config, "password").await.unwrap();

        let request_actor = RequestHandler::new(kp.key_identifier());
        let request_handler = system.create_root_actor("request", request_actor).await.unwrap();

        let event = create_gov(&kp);
    }

    fn create_gov(keys: &KeyPair) -> Signed<EventRequest> {
        let id = keys.key_identifier();
        let value = json!({
            "Create": {
                "governance_id": "",
                "schema_id": "governance",
                "namespace": "",
                "name": "EasyTutorial",
                "public_key": id
            }           
        });
        let content: EventRequest = serde_json::from_value(value).unwrap();
        let signature = Signature::new(&content, keys, DigestDerivator::SHA2_256).unwrap();
        Signed { content, signature }
    }

}