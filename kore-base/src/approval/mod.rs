// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use actor::{ActorRef, Event};
use actor::{
    Actor, ActorContext, Error as ActorError, Handler, Message, Response,
};
use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use request::ApprovalRequest;
use response::{ApprovalEntity, ApprovalResponse};
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::debug;

use crate::{SubjectCommand, SubjectResponse};
use crate::{
    db::Storable,
    evaluation::{
        self, request::EvaluationReq, response::EvaluationRes,
        EvaluationResponse,
    },
    governance::{model::Validation, Quorum},
    validation::request::ValidationReq,
    Error, Governance, Signature, Signed, Subject,
};

pub mod approver;
pub mod request;
pub mod response;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Approval {
    node_key: KeyIdentifier,
    // Quorum
    quorum: Quorum,
    // Approvals
    approvals: HashSet<KeyIdentifier>,
    // Actual responses
    approvals_response: Vec<Signature>,
    // Approvals quantity
    approvals_quantity: u32,
    // Pending event
    pending_event: Option<ApprovalEntity>,
}

impl Approval {
    pub fn new(node_key: KeyIdentifier) -> Self {
        Approval {
            node_key,
            ..Default::default()
        }
    }
    /// Delete pending approval
    fn check_validator(&mut self, approval: KeyIdentifier) -> bool {
        self.approvals.remove(&approval)
    }

    async fn create_approval_req(
        &mut self,
        request_id: DigestIdentifier,
        ctx: &mut ActorContext<Approval>,
        info: EvaluationReq,
        response: EvaluationRes,
    ) -> Result<ApprovalRequest, ActorError> {
        let subject_path = ctx.path().parent();
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        

        let subject_actor = match subject_actor {
            Some(actor) => actor,
            None => {
                return Err(ActorError::NotExists(subject_path)
                );
            }
        };

        let governance_id = subject_actor.ask(SubjectCommand::GetGovernance).await;
        match governance_id {
            Ok(SubjectResponse::Governance(gov_id)) => {
                let approval_request = ApprovalRequest {
                    event_request: info.event_request,
                    sn: info.sn,
                    gov_version: info.gov_version,
                    patch: response.patch,
                    state_hash: response.state_hash,
                    hash_prev_event: info.hash_prev_event,
                    gov_id: gov_id.get_governance_id()
                };
                Ok(approval_request)
            }
            _ => Err(ActorError::NotExists(subject_path)),
            
        }

        // Necesito obtener el sujeto
        ApprovalRequest {
            event_request: info.event_request,
            sn: info.sn,
            gov_version: info.gov_version,
            patch: response.patch,
            state_hash: response.state_hash,
            hash_prev_event: info.hash_prev_event,
            gov_id: info.gov_id,
        };

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ApprovalCommand {
    Create {
        request_id: DigestIdentifier,
        info: EvaluationReq,
    },

    Response(Signed<ApprovalResponse>),
}

impl Message for ApprovalCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalEvent {
    pub actual_event_approval_response: Vec<Signature>,
    pub approval: bool,
}

impl Event for ApprovalEvent {}

#[derive(Debug, Clone)]
pub enum ApprovalRes {
    Error(Error),
    None,
}
impl Response for ApprovalRes {}

#[async_trait]
impl Actor for Approval {
    type Event = ApprovalEvent;
    type Message = ApprovalCommand;
    type Response = ApprovalRes;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Starting approval actor with init store.");
        self.init_store("approval", false, ctx).await
        // Una vez recuperado el estado debemos ver si el propio nodo ha recibido ya ha enviado la respuesta
        //para no levantar un approver
    }
    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Stopping approval actor with stop store.");
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<Approval> for Approval {
    async fn handle_message(
        &mut self,
        msg: ApprovalCommand,
        ctx: &mut ActorContext<Self>,
    ) -> Result<ApprovalRes, Error> {
        match msg {
            ApprovalCommand::Create { request_id, info } => {
                /// Creamos una petición de aprobación, miramos quorum y lanzamos approvers
                !unimplemented!()
            }
            ApprovalCommand::Response(response) => !unimplemented!(),
        }
        Ok(ApprovalRes::None)
    }
}

#[async_trait]
impl PersistentActor for Approval {
    fn apply(&mut self, event: &ApprovalEvent) {
        self.approvals_response = event.actual_event_approval_response.clone();
    }
}

impl Storable for Approval {}
