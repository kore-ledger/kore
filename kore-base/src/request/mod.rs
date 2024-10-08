// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response,
};
use async_trait::async_trait;
use identity::{identifier::{derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier}, keys::{Ed25519KeyPair, KeyGenerator, KeyPair}};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use store::store::PersistentActor;

use crate::{
    db::Storable, governance::model::Roles, init_state, model::common::{get_gov, get_metadata}, node, subject::{self, CreateSubjectData, SubjectID}, CreateRequest, Error, Event as KoreEvent, EventRequest, HashId, Node, NodeMessage, NodeResponse, Signed, DIGEST_DERIVATOR
};

pub mod state;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestHandler {
    node_key: KeyIdentifier,
    handling: HashMap<String, String>,
    in_queue: HashMap<String, VecDeque<Signed<EventRequest>>>,
}

impl RequestHandler {
    async fn subject_owner(ctx: &mut ActorContext<RequestHandler>, subject_id: &str) -> Result<bool, Error>{
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<actor::ActorRef<Node>> = ctx.system().get_actor(&node_path).await;

        let response = if let Some(node_actor) = node_actor {
            if let Ok(response) = node_actor.ask(NodeMessage::AmISubjectOwner(subject_id.to_owned())).await {
                response
            } else {
                todo!()
            }
        } else {
            todo!()
        };

        match response {
            NodeResponse::AmIOwner(owner) => Ok(owner),
            _ => todo!()
        }
    }

    async fn queued_event(ctx: &mut ActorContext<RequestHandler>, subject_id: &str) -> Result<(), Error> {
        let request_path = ActorPath::from("/user/request_handler");
        let request_actor: Option<actor::ActorRef<RequestHandler>> = ctx.system().get_actor(&request_path).await;

        if let Some(request_actor) = request_actor {
            if let Err(e) = request_actor.tell(RequestHandlerCommand::PopQueue { subject_id: subject_id.to_owned() }).await {
                todo!()
            }
        }
        Ok(())
    }

    async fn create_subject(ctx: &mut ActorContext<RequestHandler>, create_req: CreateRequest, request: Signed<EventRequest>) -> Result<DigestIdentifier, Error> {
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

        let subject_id =  subject_id.hash_id(derivator)?;

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
            let governance = get_gov(ctx, create_req.governance_id.clone()).await?;
            let value = governance.get_init_state(&create_req.schema_id)?;

            CreateSubjectData {
                keys,
                create_req,
                subject_id: subject_id,
                creator: request.signature.signer.clone(),
                genesis_gov_version: governance.get_version(),
                value,
            }            
        };

        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<actor::ActorRef<Node>> = ctx.system().get_actor(&node_path).await;

        let response = if let Some(node_actor) = node_actor {
            if let Ok(response) = node_actor.ask(NodeMessage::CreateNewSubjectReq(data.clone())).await {
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
            _ => todo!()
        }
    }
}

#[derive(Debug, Clone)]
pub enum RequestHandlerCommand {
    NewRequest { request: Signed<EventRequest> },
    PopQueue {
        subject_id: String
    },
    EndHandling {
        subject_id: String
    },
    GetState { request_id: String },
}

impl Message for RequestHandlerCommand {}

#[derive(Debug, Clone)]
pub enum RequestHandlerResponse {
    Ok(String),
    Error(Error),
    None
}

impl Response for RequestHandlerResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestHandlerEvent {
    EventToQueue {
        subject_id: String,
        event: Signed<EventRequest>
    },
    FinishHandling {
        subject_id: String,
    },
    EventToHandling {
        subject_id: String,
        request_id: String
    }
}

impl Event for RequestHandlerEvent {}

#[async_trait]
impl Actor for RequestHandler {
    type Event = RequestHandlerEvent;
    type Message = RequestHandlerCommand;
    type Response = RequestHandlerResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.init_store("request", None, false, ctx).await
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await?;
        Ok(())
    }
}

#[async_trait]
impl PersistentActor for RequestHandler {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) {
        match event {
            RequestHandlerEvent::EventToQueue { subject_id, event } => {
                if let Some(vec) = self.in_queue.get_mut(subject_id) {
                    vec.push_back(event.clone());
                } else {
                    self.in_queue.insert(subject_id.clone(), VecDeque::new());
                };
            },
            RequestHandlerEvent::EventToHandling { subject_id, request_id } => {
                self.handling.insert(subject_id.clone(), request_id.clone());
                if let Some(vec) = self.in_queue.get_mut(subject_id) {
                    vec.pop_front();
                }
            },
            RequestHandlerEvent::FinishHandling { subject_id } => {
                self.handling.remove(subject_id);
            }
        }
    }
}

#[async_trait]
impl Handler<RequestHandler> for RequestHandler {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: RequestHandlerCommand,
        ctx: &mut actor::ActorContext<RequestHandler>,
    ) -> Result<RequestHandlerResponse, ActorError> {
        match msg {
            RequestHandlerCommand::NewRequest { request } => {
                // Comprobamos que la firma sea correcta.
                // Si no hay ninguna actualmente para ese sujeto:
                //      - Verificamos el tipo de eventos y permisos para ese evento.
                //      - Si no es un evento de creación tenemos que verificar que tenemos a ese sujeto como owner. Si fuese un evento de confirmación y no somos owner nos intentamos actualizar.
                //
                // Si hay alguna, simplemente la encolamos.
                if let Err(e) = request.verify() {
                    todo!()
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

                        if create_request.name.is_empty() {
                            return Ok(RequestHandlerResponse::Error(
                                Error::RequestHandler(
                                    "The name cannot be empty.".to_owned(),
                                ),
                            ));
                        }

                        if create_request.schema_id == "governance" {
                            if !create_request.namespace.is_empty() {
                                return Ok(RequestHandlerResponse::Error(Error::RequestHandler("The creation event is for a governance, the namespace must be empty.".to_owned())));
                            }

                            if !create_request.governance_id.digest.is_empty() {
                                return Ok(RequestHandlerResponse::Error(Error::RequestHandler("The creation event is for a governance, the governance_id must be empty.".to_owned())));
                            }
                        } else {
                            if create_request.governance_id.digest.is_empty() {
                                return Ok(RequestHandlerResponse::Error(Error::RequestHandler("The creation event is for a traceability subject, the governance_id cannot be empty.".to_owned())));
                            }

                            let gov = match get_gov(ctx, create_request.governance_id.clone()).await {
                                Ok(gov) => gov,
                                Err(e) => return Ok(RequestHandlerResponse::Error(Error::RequestHandler(format!("It has not been possible to obtain governance: {}", e)))),
                            };

                            if !gov
                                .get_signers(
                                    Roles::CREATOR { quantity: 0 },
                                    &create_request.schema_id,
                                    create_request.namespace.clone(),
                                )
                                .iter()
                                .any(|x| x.clone() == self.node_key)
                            {
                                return Ok(RequestHandlerResponse::Error(Error::RequestHandler("The Scheme does not exist or does not have permissions for the creation of subjects, it needs to be assigned the creator role.".to_owned())));
                            };
                        }
                        let subject_id = match RequestHandler::create_subject(ctx, create_request, request.clone()).await {
                            Ok(subject_id) => subject_id,
                            Err(e) => return Ok(RequestHandlerResponse::Error(Error::RequestHandler(format!("An error has occurred and the subject could not be created: {}", e))))
                        };

                        self.on_event(RequestHandlerEvent::EventToQueue { subject_id: subject_id.to_string(), event: request }, ctx).await;

                        if self.handling.get(&subject_id.to_string()).is_none() {
                            if let Err(e) = RequestHandler::queued_event(ctx, &subject_id.to_string()).await {
                                todo!()
                            }
                        }

                        return Ok(RequestHandlerResponse::Ok("The event has been successfully queued.".to_owned()))
                    }
                    EventRequest::Fact(fact_request) => fact_request.subject_id,
                    EventRequest::Transfer(transfer_request) => {
                        transfer_request.subject_id
                    }
                    EventRequest::Confirm(confirm_request) => {
                        confirm_request.subject_id
                    }
                    EventRequest::EOL(eol_request) => eol_request.subject_id,
                };

                if subject_id.digest.is_empty() {
                    return Ok(RequestHandlerResponse::Error(Error::RequestHandler("Subject_id cannot be empty.".to_owned())));
                }

                // Primero check que seamos el owner.
                let owner = match Self::subject_owner(ctx, &subject_id.to_string()).await {
                    Ok(owner) => owner,
                    Err(e) => return Ok(RequestHandlerResponse::Error(Error::RequestHandler(format!("An error has occurred: {}", e))))
                };

                if !owner {
                    if let EventRequest::Confirm(confirm_request) =  request.content.clone() {
                        // TODO VAMOS A Intentar actualizarnos, a lo mejor se ha hecho un evento de transferencia pero no lo hemos recibido.
                    } else {
                        return Ok(RequestHandlerResponse::Error(Error::RequestHandler("An event is being sent for a subject that does not belong to us.".to_owned())))
                    }
                }

                self.on_event(RequestHandlerEvent::EventToQueue { subject_id: subject_id.to_string(), event: request }, ctx).await;

                if self.handling.get(&subject_id.to_string()).is_none() {
                    if let Err(e) = RequestHandler::queued_event(ctx, &subject_id.to_string()).await {
                        todo!()
                    }
                }

                Ok(RequestHandlerResponse::Ok("The event has been successfully queued.".to_owned()))
            }
            RequestHandlerCommand::GetState { request_id } => todo!(),
            RequestHandlerCommand::PopQueue { subject_id } => {

                let event = if let Some(events) = self.in_queue.get(&subject_id) {
                    if let Some(event) = events.clone().pop_front() {
                        event
                    } else {
                        // No hay más eventos pendientes.
                        return Ok(RequestHandlerResponse::None);
                    }
                } else {
                    // TODO es imposible que sea un option
                    todo!()
                };

                
                
                Ok(RequestHandlerResponse::None)
            },
            RequestHandlerCommand::EndHandling { subject_id } => {
                self.on_event(RequestHandlerEvent::FinishHandling { subject_id: subject_id.clone() } , ctx).await;

                if let Err(e) = RequestHandler::queued_event(ctx, &subject_id).await {
                    todo!()
                }

                Ok(RequestHandlerResponse::None)
            },
        }
    }

    async fn on_event(
        &mut self,
        event: RequestHandlerEvent,
        ctx: &mut ActorContext<RequestHandler>,
    ) {
        if let Err(e) = self.persist(&event, ctx).await {
            // TODO Propagar error.
        };
    }
}

#[async_trait]
impl Storable for RequestHandler {}
