use std::f64::consts::E;

use crate::{
    db::Storable,
    intermediary::Intermediary,
    model::{network::RetryNetwork, SignTypesNode},
    ActorMessage, Error, EventRequest, Governance, NetworkMessage, Node,
    NodeMessage, NodeResponse, Signature, Signed, Subject, SubjectCommand,
    SubjectResponse, DIGEST_DERIVATOR,
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    ExponentialBackoffStrategy, Handler, Message, Response, RetryActor,
    RetryMessage, Strategy,
};
use async_trait::async_trait;
use identity::identifier::{
    derive::digest::DigestDerivator, Derivable, DigestIdentifier, KeyIdentifier,
};
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::error;

use super::{
    request::ApprovalRequest,
    response::{
        ApprovalEntity, ApprovalError, ApprovalResponse, ApprovalState,
        VotationType,
    },
    Approval, ApprovalCommand, ApprovalRes,
};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Approver {
    // TODO: Añadir derivator
    node: KeyIdentifier,
    request_id: String,
    pass_votation: VotationType,
    request: Option<ApprovalEntity>,
}

fn subject_id_by_request(
    request: &EventRequest,
    gov_version: &u64,
    derivator: DigestDerivator,
) -> Result<DigestIdentifier, Error> {
    let subject_id = match request {
        EventRequest::Fact(ref fact_request) => fact_request.subject_id.clone(),
        EventRequest::Create(ref create_request) => {
            DigestIdentifier::from_serializable_borsh(
                (
                    &create_request.namespace,
                    &create_request.schema_id,
                    &create_request.public_key.to_str(),
                    &create_request.governance_id.to_str(),
                    gov_version,
                    derivator,
                ),
                derivator,
            )
            .map_err(|_| Error::Approval("()".to_string()))?
        }
        _ => {
            return Err(Error::Approval(
                "Only events of type Create and Fact are allowed.".to_string(),
            ))
        }
    };
    Ok(subject_id)
}

impl Approver {
    pub fn new(request_id: String, node: KeyIdentifier) -> Self {
        Approver {
            node,
            request_id,
            ..Default::default()
        }
    }
    // Refactorizar el get_gov se usa en todos los procesos
    async fn get_gov(
        &self,
        ctx: &mut ActorContext<Approver>,
        governance_id: DigestIdentifier,
    ) -> Result<Governance, Error> {
        // Governance path
        let governance_path =
            ActorPath::from(format!("/user/node/{}", governance_id));

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
            SubjectResponse::Error(error) => {
                return Err(Error::Actor(format!("The subject encountered problems when getting governance: {}",error)));
            }
            _ => {
                return Err(Error::Actor(format!(
                    "An unexpected response has been received from node actor"
                )))
            }
        }
    }

    async fn approval(
        &mut self,
        ctx: &mut ActorContext<Approver>,
        approval_request: Signed<ApprovalRequest>,
    ) -> Result<(), Error> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            derivator.clone()
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };
        // Obtengo el subject id de la request(FACT, CREATE(la genero))
        //let subject_id = subject_id_by_request(&approval_request.event_request.content,&approval_request.gov_version, derivator)?;

        let schema_id = match approval_request.content.event_request.content {
            EventRequest::Create(ref create_request) => {
                create_request.schema_id.as_str()
            }
            _ => "",
        };

        // Verifico si esoty actualizado
        let gov_id = approval_request.content.governance_id.clone();
        let governance = self.get_gov(ctx, gov_id).await?;

        match approval_request
            .content
            .gov_version
            .cmp(&governance.get_version())
        {
            std::cmp::Ordering::Equal => {
                // If it is the same it means that we have the latest version of governance, we are up to date.
            }
            std::cmp::Ordering::Greater => {
                // It is impossible to have a greater version of governance than the owner of the governance himself.
                // The only possibility is that it is an old approval request.
                // Hay que hacerlo TODO
            }
            std::cmp::Ordering::Less => {
                // Si es un sujeto de traabilidad hay que darle una vuelta.
                // Stop evaluation process, we need to update governance, we are out of date.
                // Hay que hacerlo TODO
            }
        }
        // identificador de la nueva request | forma de preguntar desde la api
        let id: DigestIdentifier =
            match DigestIdentifier::generate_with_blake3(&approval_request)
                .map_err(|_| Error::Approval("Error generating id".to_string()))
            {
                Ok(id) => id,
                Err(error) => return Err(error),
            };
        // actualizamos el estado del actor y ponemos en pendiente
        self.request = Some(ApprovalEntity {
            id: id.clone(),
            request: approval_request,
            state: ApprovalState::Pending,
        });
        Ok(())
    }
    // Nunca se va a poder emitir un voto para una request que no existe
    async fn generate_vote(
        &self,
        ctx: &mut ActorContext<Approver>,
        pending_request_id: &DigestIdentifier,
        acceptance: bool,
    ) -> Result<Signature, Error> {
        // Node path.
        let node_path = ActorPath::from("/user/node");
        // Node actor.
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        // We obtain the actor node
        let request_content = if let Some(request) = self.request.as_ref() {
            request.request.clone().content
        } else {
            return Err(Error::Actor("Request content is missing".into()));
        };

        let response = if let Some(node_actor) = node_actor {
            // We ask a node
            let response = node_actor
                .ask(NodeMessage::SignRequest(SignTypesNode::ApprovalReq(
                    request_content,
                )))
                .await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a node: {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The node actor was not found in the expected path /user/node"
            )));
        };

        // We handle the possible responses of node
        let response = match response {
            NodeResponse::SignRequest(sign) => Ok(sign),
            NodeResponse::Error(error) => Err(Error::Actor(format!(
                "The node encountered problems when signing the proof: {}",
                error
            ))),
            _ => Err(Error::Actor(format!(
                "An unexpected response has been received from node actor"
            ))),
        };
        response
    }
}

#[derive(Debug, Clone)]
pub enum ApproverCommand {
    // Mensaje para aprobar localmente
    LocalApprover {
        approval_req: Signed<ApprovalRequest>,
        our_key: KeyIdentifier,
    },
    // Lanza los retries y envía la petición a la network(exponencial)
    NetworkApprover {
        request_id: String,
        approval_req: Signed<ApprovalRequest>,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    // Finaliza los retries y recibe la respuesta de la network
    NetworkResponse {
        approval_res: Signed<ApprovalResponse>,
        request_id: String,
    },
    // Mensaje para pedir aprobación desde el helper y devolver ahi
    NetworkRequest {
        approval_req: Signed<ApprovalRequest>,
        info: ComunicateInfo,
    },
    // Necesito poder emitir un evento de aprobación, no solo el automático
}

impl Message for ApproverCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct  ApproverEvent {
    pub pending_event: ApprovalEntity
}

impl Event for ApproverEvent {}

#[derive(Debug, Clone)]
pub enum ApproverResponse {
    None,
}

impl Response for ApproverResponse {}

#[async_trait]
impl Actor for Approver {
    type Event = ApproverEvent;
    type Message = ApproverCommand;
    type Response = ApproverResponse;
}

#[async_trait]
impl Handler<Approver> for Approver {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: ApproverCommand,
        ctx: &mut ActorContext<Approver>,
    ) -> Result<ApproverResponse, ActorError> {
        match msg {
            // aprobar si esta por defecto
            ApproverCommand::LocalApprover {
                approval_req,
                our_key,
            } => {
                // generamos el estado
                if let Err(e) = self.approval(ctx, approval_req).await {
                    todo!()
                } else if self.pass_votation == VotationType::AlwaysAccept {
                    let request_id = match self.request.as_ref() {
                        Some(value) => value,
                        None => {
                            return Err(ActorError::Get(
                                "Request is missing".to_string(),
                            ));
                        }
                    };
                    // si se ha generado correctamente y aprobamos por defecto
                    let approval = match self
                        .generate_vote(ctx, &request_id.id, true)
                        .await
                    {
                        Ok(approver) => ApprovalCommand::Response {
                            approval_res: ApprovalResponse::Signature(
                                approver, true,
                            ),
                            sender: our_key.clone(),
                        },
                        Err(e) => {
                            // Log con el error. TODO
                            ApprovalCommand::Response {
                                approval_res: ApprovalResponse::Error(
                                    ApprovalError {
                                        who: our_key.clone(),
                                        error: format!(
                                            "Error generating vote: {}",
                                            e
                                        ),
                                    },
                                ),
                                sender: our_key,
                            }
                        }
                    };
                    // Approval Path
                    let approval_path = ctx.path().parent();
                    // Approval actor.
                    let approval_actor: Option<ActorRef<Approval>> =
                        ctx.system().get_actor(&approval_path).await;
                    // Send response of validation to parent
                    if let Some(approval_actor) = approval_actor {
                        if let Err(e) = approval_actor.tell(approval).await {
                            return Err(e);
                        }
                    } else {
                        // Can not obtain parent actor
                        return Err(ActorError::Exists(approval_path));
                    }

                    ctx.stop().await;
                } else {
                    todo!()
                }
                todo!()
            }
            ApproverCommand::NetworkApprover {
                request_id,
                approval_req,
                node_key,
                our_key,
            } => {
                // Solo admitimos eventos FACT
                let subject_id = if let EventRequest::Fact(event) =
                    approval_req.content.event_request.content.clone()
                {
                    event.subject_id
                } else {
                    todo!()
                };
                let reciver_actor =
                    format!("/user/node/{}/approver", subject_id);

                // Lanzar evento donde lanzar los retrys
                let message = NetworkMessage {
                    info: ComunicateInfo {
                        request_id,
                        sender: our_key,
                        reciver: node_key,
                        reciver_actor,
                        schema: "".to_string(),
                    },
                    message: ActorMessage::ApproverReq(approval_req),
                };
                let target = RetryNetwork::default();

                // Estrategia exponencial
                let strategy = Strategy::ExponentialBackoff(
                    ExponentialBackoffStrategy::new(6),
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

                todo!()
            }
            // Finaliza los retries
            ApproverCommand::NetworkResponse {
                approval_res,
                request_id,
            } => {
                if request_id == self.request_id {
                    if self.node != approval_res.signature.signer {
                        // Nos llegó a una aprobación de un nodo incorrecto!
                        todo!()
                    }
                    if let Err(e) = approval_res.verify() {
                        // Hay error criptográfico en la respuesta
                        todo!()
                    }

                    // Approval path.
                    let approval_path = ctx.path().parent();

                    // Approval actor.
                    let approval_actor: Option<ActorRef<Approval>> =
                        ctx.system().get_actor(&approval_path).await;

                    if let Some(approval_actor) = approval_actor {
                        if let Err(e) = approval_actor
                            .tell(ApprovalCommand::Response {
                                approval_res: approval_res.content,
                                sender: self.node.clone(),
                            })
                            .await
                        {
                            // TODO error, no se puede enviar la response. Parar
                        }
                    } else {
                        // TODO no se puede obtener aprobación! Parar.
                        // Can not obtain parent actor
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
                } else {
                    // TODO llegó una respuesta con una request_id que no es la que estamos esperando, no es válido.
                }
                !todo!()
            }
            ApproverCommand::NetworkRequest { approval_req, info } => {
                if info.schema == "governance" {
                    // Aquí hay que comprobar que el owner del subject es el que envía la req.
                    let subject_path = ActorPath::from(format!(
                        "/user/node/{}",
                        approval_req.content.governance_id
                    ));
                    let subject_actor: Option<ActorRef<Subject>> =
                        ctx.system().get_actor(&subject_path).await;

                    // We obtain the evaluator
                    let response = if let Some(subject_actor) = subject_actor {
                        match subject_actor.ask(SubjectCommand::GetOwner).await
                        {
                            Ok(response) => response,
                            Err(e) => todo!(),
                        }
                    } else {
                        todo!()
                    };

                    let subject_owner = match response {
                        SubjectResponse::Owner(owner) => owner,
                        _ => todo!(),
                    };

                    if subject_owner != approval_req.signature.signer {
                        // Error nos llegó una evaluation req de un nodo el cual no es el dueño
                        todo!()
                    }

                    if let Err(e) = approval_req.verify() {
                        // Hay errores criptográficos
                        todo!()
                    }
                }
                // Llegados a este punto se ha verificado que la req es del owner del sujeto y está todo correcto.
                // Validar y devolver la respuesta al helper, no a Validation. Nos llegó por la network la validación.
                // Sacar el Helper aquí

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("NetworkIntermediary").await;
                let mut helper = if let Some(helper) = helper {
                    helper
                } else {
                    // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
                    // return Err(ActorError::Get("Error".to_owned()))
                    return Err(ActorError::NotHelper);
                };

                let EventRequest::Fact(state_data) =
                    &approval_req.content.event_request.content
                else {
                    // Manejar, solo se aprueban los eventos de tipo fact TODO
                    todo!()
                };

                if let Err(e) = self.approval(ctx, approval_req.clone()).await {
                    todo!()
                } else if self.pass_votation == VotationType::AlwaysAccept {
                    let approval = match self
                        .generate_vote(ctx, &DigestIdentifier::default(), true)
                        .await
                    {
                        Ok(approval) => {
                            ApprovalResponse::Signature(approval, true)
                        }
                        Err(e) => ApprovalResponse::Error(ApprovalError {
                            who: self.node.clone(),
                            error: format!("Error generating vote: {}", e),
                        }),
                    };

                    let new_info = ComunicateInfo {
                        reciver: info.sender,
                        sender: info.reciver.clone(),
                        request_id: info.request_id,
                        reciver_actor: format!(
                            "/user/node/{}/approval/{}",
                            approval_req.content.governance_id,
                            info.reciver.clone()
                        ),
                        schema: info.schema.clone(),
                    };

                    // Aquí tiene que firmar el nodo la respuesta.
                    let node_path = ActorPath::from("/user/node");
                    let node_actor: Option<ActorRef<Node>> =
                        ctx.system().get_actor(&node_path).await;

                    // We obtain the approval
                    let node_response = if let Some(node_actor) = node_actor {
                        match node_actor
                            .ask(NodeMessage::SignRequest(
                                SignTypesNode::ApprovalRes(approval.clone()),
                            ))
                            .await
                        {
                            Ok(response) => response,
                            Err(e) => todo!(),
                        }
                    } else {
                        todo!()
                    };

                    let signature = match node_response {
                        NodeResponse::SignRequest(signature) => signature,
                        NodeResponse::Error(_) => todo!(),
                        _ => todo!(),
                    };
    
                    let signed_response: Signed<ApprovalResponse> = Signed {
                        content: approval,
                        signature,
                    };
    
                    if let Err(e) = helper
                        .send_command(network::CommandHelper::SendMessage {
                            message: NetworkMessage {
                                info: new_info,
                                message: ActorMessage::ApproverRes(
                                    signed_response,
                                ),
                            },
                        })
                        .await
                    {
                        // error al enviar mensaje, propagar hacia arriba TODO
                    };
                }

                todo!()
            }
        }
    }
}

// Debemos persistir el estado de la petición hasta que se apruebe
#[async_trait]
impl PersistentActor for Approver {
    fn apply(&mut self, event: &ApproverEvent) {
        self.request = Some(event.pending_event.clone());
    }
}

impl Storable for Approver {}
