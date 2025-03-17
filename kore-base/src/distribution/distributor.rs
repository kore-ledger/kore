// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::Duration;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, ChildAction, Error as ActorError,
    FixedIntervalStrategy, Handler, Message, RetryActor, RetryMessage,
    Strategy,
};
use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;

use crate::{
    auth::WitnessesAuth, governance::{
        model::{CreatorQuantity, Roles}, Governance
    }, intermediary::Intermediary, model::{
        common::{
            emit_fail, get_gov, get_metadata, get_quantity, subject_old_owner,
            try_to_update, update_event, update_vali_data,
        },
        event::{Ledger, ProtocolsSignatures},
        network::RetryNetwork, Namespace,
    }, update::TransferResponse, validation::proof::ValidationProof, ActorMessage, Event as KoreEvent, EventRequest, NetworkMessage, Node, NodeMessage, NodeResponse, Signed, Subject, SubjectMessage, SubjectResponse
};

use tracing::{error, warn};

const TARGET_DISTRIBUTOR: &str = "Kore-Distribution-Distributor";

use super::{Distribution, DistributionMessage};

pub struct Distributor {
    pub node: KeyIdentifier,
}

impl Distributor {
    async fn get_subject(
        &self,
        ctx: &mut ActorContext<Distributor>,
        subject_id: DigestIdentifier,
    ) -> Result<ActorRef<Subject>, ActorError> {
        let subject_path =
            ActorPath::from(format!("/user/node/{}", subject_id));
        let subject_actor: ActorRef<Subject> = if let Some(subject_actor) =
            ctx.system().get_actor(&subject_path).await
        {
            subject_actor
        } else {
            return Err(ActorError::NotFound(subject_path));
        };

        Ok(subject_actor)
    }

    async fn create_subject(
        &self,
        ctx: &mut ActorContext<Distributor>,
        ledger: Signed<Ledger>,
    ) -> Result<(), ActorError> {
        if let EventRequest::Create(request) =
            ledger.content.event_request.content.clone()
        {
            if request.schema_id != "governance" {
                let gov =
                    get_gov(ctx, &request.governance_id.to_string()).await?;

                if let Some(max_quantity) = gov.max_creations(
                    &ledger.signature.signer.to_string(),
                    &request.schema_id,
                    request.namespace.clone(),
                ) {
                    let quantity = get_quantity(
                        ctx,
                        request.governance_id.to_string(),
                        request.schema_id.clone(),
                        ledger.signature.signer.to_string(),
                        request.namespace.to_string(),
                    )
                    .await?;

                    if let CreatorQuantity::QUANTITY(max_quantity) =
                        max_quantity
                    {
                        if quantity >= max_quantity as usize {
                            return Err(ActorError::Functional("The maximum number of created subjects has been reached".to_owned()));
                        }
                    }
                } else {
                    return Err(ActorError::Functional("The number of subjects that can be created has not been found".to_owned()));
                };
            }
        }

        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        let response = if let Some(node_actor) = node_actor {
            node_actor
                .ask(NodeMessage::CreateNewSubjectLedger(ledger))
                .await?
        } else {
            return Err(ActorError::NotFound(node_path));
        };
        match response {
            NodeResponse::SonWasCreated => Ok(()),
            _ => Err(ActorError::UnexpectedResponse(
                node_path,
                "NodeResponse::SonWasCreated".to_owned(),
            )),
        }
    }

    async fn get_ledger(
        &self,
        ctx: &mut ActorContext<Distributor>,
        subject_id: &str,
        last_sn: u64,
    ) -> Result<
        (
            Vec<Signed<Ledger>>,
            Option<Signed<KoreEvent>>,
            Option<ValidationProof>,
            Option<Vec<ProtocolsSignatures>>,
        ),
        ActorError,
    > {
        let subject_path =
            ActorPath::from(format!("/user/node/{}", subject_id));
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        let response = if let Some(subject_actor) = subject_actor {
            // We ask a node
            subject_actor
                .ask(SubjectMessage::GetLedger { last_sn })
                .await?
        } else {
            return Err(ActorError::NotFound(subject_path));
        };

        match response {
            SubjectResponse::Ledger {
                ledger,
                last_event,
                last_proof,
                prev_event_validation_response,
            } => Ok((
                ledger,
                last_event,
                *last_proof,
                prev_event_validation_response,
            )),
            _ => Err(ActorError::UnexpectedResponse(
                subject_path,
                "SubjectResponse::Ledger".to_owned(),
            )),
        }
    }

    async fn authorized_subj(
        &self,
        ctx: &mut ActorContext<Distributor>,
        subject_id: &str,
    ) -> Result<(bool, bool, bool), ActorError> {
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        let response = if let Some(node_actor) = node_actor {
            node_actor
                .ask(NodeMessage::IsAuthorized(subject_id.to_owned()))
                .await?
        } else {
            return Err(ActorError::NotFound(node_path));
        };
        match response {
            NodeResponse::IsAuthorized { owned, auth, know } => Ok((owned, auth, know)),
            _ => Err(ActorError::UnexpectedResponse(
                node_path,
                "NodeResponse::IsAuthorized".to_owned(),
            )),
        }
    }

    async fn check_auth(
        &self,
        ctx: &mut ActorContext<Distributor>,
        signer: KeyIdentifier,
        info: ComunicateInfo,
        ledger: Ledger,
        gov_version: Option<u64>,
        schema: &str,
        namespace: Namespace,
        governance_id: DigestIdentifier
    ) -> Result<(), ActorError> {
        let our_key = info.reciver.clone();
        let subject_id = ledger.subject_id.clone();
        // Si está auth o si soy el dueño del sujeto.
        let (owned, auth, know) = self.authorized_subj(ctx, &subject_id.to_string()).await?;

        // Si es una gov
        let is_gov = schema == "governance";

        // es gov
        if is_gov {
            // No está auth
            if !auth {
                return Err(ActorError::Functional(
                    "Governance is not authorized".to_owned(),
                ))
            // está auth y tengo la copia
            } else if owned || know  {
                return Ok(());
            // está auth pero no tengo la copia
            } else if let EventRequest::Create(_) = ledger.event_request.content.clone()
                {
                    return Ok(());
                } else {
                    try_to_update(ctx, subject_id, WitnessesAuth::Owner(signer)).await?;
                    return Err(ActorError::Functional(
                        "Updating governance".to_owned(),
                    ));
                }
            
        }

        // no es gov.
        let (namespace, governance_id, update) =
            match get_metadata(ctx, &subject_id.to_string()).await {
                Ok(metadata) => (metadata.namespace, metadata.governance_id, false),
                Err(e) => {
                    if let ActorError::NotFound(_) = e {
                        if let EventRequest::Create(request) =
                            ledger.event_request.content.clone()
                        {
                            (request.namespace, request.governance_id, false)
                        } else {
                            (namespace, governance_id, true)
                        }
                    } else {
                        return Err(e);
                    }
                }
            };

            // obtenemos la gov.
            let gov = match get_gov(ctx, &governance_id.to_string()).await {
                Ok(gov) => gov,
                Err(e) => {
                    if let ActorError::NotFound(_) = e {
                        try_to_update(ctx, governance_id, WitnessesAuth::Witnesses)
                            .await?;
                        return Err(ActorError::Functional(
                            "Updating governance".to_owned(),
                        ));
                    }
                    return Err(e);
                }
            };

            // Comparamos las govs, puede ser que ya no seamos testigo o que de repente seamos testigos.
            if let Some(gov_version) = gov_version {
                Self::cmp_govs(
                    ctx,
                    gov.clone(),
                    gov_version,
                    governance_id,
                    info,
                    schema
                )
                .await?;
            }

            // Si no está autorizado explicitamente.
            if !auth {
                // Miramos que tengamos el rol.
                if !gov.has_this_role(
                    &signer.to_string(),
                    Roles::CREATOR(CreatorQuantity::QUANTITY(0)),
                    schema,
                    namespace.clone(),
                ) {
                    return Err(ActorError::Functional(
                        "Signer is not a creator".to_string(),
                    ));
                }

                if !gov.has_this_role(
                    &our_key.to_string(),
                    Roles::WITNESS,
                    schema,
                    namespace,
                ) {
                    return Err(ActorError::Functional(
                        "We are not witness".to_string(),
                    ));
                }
            }

            if update {
                try_to_update(ctx, subject_id, WitnessesAuth::Owner(signer))
                .await?;
                return Err(ActorError::Functional(
                    "Updating subject".to_owned(),
                ));
            }

            Ok(())
    }

    async fn cmp_govs(
        ctx: &mut ActorContext<Distributor>,
        gov: Governance,
        gov_version: u64,
        governance_id: DigestIdentifier,
        info: ComunicateInfo,
        schema: &str
    ) -> Result<(), ActorError> {
        let governance_id_string = governance_id.to_string();

        let our_gov_version = gov.version;
        match our_gov_version.cmp(&gov_version) {
            std::cmp::Ordering::Less => {
                // Mi version es menor, me actualizo. y no le envío nada
                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver,
                    request_id: info.request_id,
                    version: info.version,
                    reciver_actor: format!(
                        "/user/node/{}/distributor",
                        governance_id_string
                    )
                };

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    return Err(ActorError::NotHelper("network".to_owned()));
                };

                helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::DistributionLedgerReq {
                                gov_version: Some(our_gov_version),
                                actual_sn: Some(our_gov_version),
                                subject_id: governance_id,
                            },
                        },
                    })
                    .await?;

                return Err(ActorError::Functional(
                    "My gov version is less than his gov version".to_owned(),
                ));
            }
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Greater => {
                // Su version es menor. Traslado la solicitud de actualización
                // a mi distributor gov.
                if schema != "governance" {
                    let new_info = ComunicateInfo {
                        reciver: info.reciver,
                        sender: info.sender,
                        request_id: info.request_id,
                        version: info.version,
                        reciver_actor: format!(
                            "/user/node/{}/distributor",
                            governance_id_string
                        ),
                    };

                    let distributor_path = ActorPath::from(format!(
                        "/user/node/{}/distributor",
                        governance_id_string
                    ));

                    let Some(distributor_actor): Option<ActorRef<Distributor>> =
                        ctx.system().get_actor(&distributor_path).await
                    else {
                        let e = ActorError::NotFound(distributor_path);
                        return Err(emit_fail(ctx, e).await);
                    };

                    distributor_actor
                        .tell(DistributorMessage::SendDistribution {
                            gov_version: Some(gov_version),
                            actual_sn: Some(gov_version),
                            subject_id: governance_id_string,
                            info: new_info,
                        })
                        .await?;
                    warn!(TARGET_DISTRIBUTOR, "His gov version is less than my gov version");
                }
            }
        }

        Ok(())
    }

    async fn check_gov_version(
        &self,
        ctx: &mut ActorContext<Distributor>,
        subject_id: &str,
        gov_version: Option<u64>,
        info: &ComunicateInfo,
    ) -> Result<(), ActorError> {
        let gov = match get_gov(ctx, subject_id).await {
            Ok(gov) => gov,
            Err(e) => {
                if let ActorError::NotFound(_) = e {
                    return Err(ActorError::Functional(
                        "Can not get governance".to_owned(),
                    ));
                } else {
                    return Err(e);
                }
            }
        };

        let metadata = match get_metadata(ctx, subject_id).await {
            Ok(metadata) => metadata,
            Err(e) => {
                if let ActorError::NotFound(_) = e {
                    return Err(ActorError::Functional(
                        "Can not get metadata".to_owned(),
                    ));
                } else {
                    return Err(e);
                }
            }
        };

        let governance_id = if metadata.governance_id.is_empty() {
            metadata.subject_id
        } else {
            metadata.governance_id
        };

        if let Some(gov_version) = gov_version {
            Self::cmp_govs(
                ctx,
                gov.clone(),
                gov_version,
                governance_id,
                info.clone(),
                &metadata.schema_id
            )
            .await?;
        }

        if let Some(new_owner) = metadata.new_owner {
            if metadata.owner == info.sender || new_owner == info.sender {
                return Ok(());
            }
        } else if metadata.owner == info.sender {
            return Ok(());
        }

        if !gov.has_this_role(
            &info.sender.to_string(),
            Roles::WITNESS,
            &metadata.schema_id.clone(),
            metadata.namespace.clone(),
        ) {
            return Err(ActorError::Functional(
                "Sender is neither a witness nor an owner nor a new owner "
                    .to_owned(),
            ));
        };

        Ok(())
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
    Transfer {
        subject_id: String,
        info: ComunicateInfo,
    },
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
        request_id: String,
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
        last_proof: ValidationProof,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
    },
    // El nodo al que le enviamos la replica la recivió, parar los reintentos.
    NetworkResponse {
        signer: KeyIdentifier,
    },
    // Nos llega una replica, guardarla en informar que la hemos recivido
    LastEventDistribution {
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>,
        last_proof: ValidationProof,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
        info: ComunicateInfo,
    },
    LedgerDistribution {
        events: Vec<Signed<Ledger>>,
        last_event: Option<Signed<KoreEvent>>,
        last_proof: Option<ValidationProof>,
        prev_event_validation_response: Option<Vec<ProtocolsSignatures>>,
        namespace: Namespace,
        schema: String,
        governance_id: DigestIdentifier,
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
            DistributorMessage::Transfer { subject_id, info } => {
                let metadata = match get_metadata(ctx, &subject_id).await {
                    Ok(metadata) => metadata,
                    Err(e) => {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "Transfer, Can not get metadata: {}", e
                        );
                        if let ActorError::NotFound(_) = e {
                            return Err(e);
                        } else {
                            return Err(emit_fail(ctx, e).await);
                        }
                    }
                };

                if let Some(_new_owner) = metadata.new_owner {
                    if metadata.owner == info.sender {
                        // Todavía no se ha emitido evento de confirm ni de reject
                        return Ok(());
                    }
                }

                let is_old_owner = match subject_old_owner(
                    ctx,
                    &subject_id.to_string(),
                    info.sender.clone(),
                )
                .await
                {
                    Ok(is_old_owner) => is_old_owner,
                    Err(e) => {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "Transfer, Can not know if is old owner: {}", e
                        );
                        if let ActorError::NotFound(_) = e {
                            return Err(e);
                        } else {
                            return Err(emit_fail(ctx, e).await);
                        }
                    }
                };

                let res = if is_old_owner {
                    TransferResponse::Confirm
                } else if !is_old_owner && metadata.owner == info.sender.clone()
                {
                    TransferResponse::Reject
                } else {
                    let e = "Sender is not the owner and is not a old owner";
                    error!(TARGET_DISTRIBUTOR, "Transfer, {}", e);

                    return Err(ActorError::Functional(e.to_owned()));
                };

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    version: info.version,
                    reciver_actor: format!(
                        "/user/node/auth/transfer_{}/{}",
                        subject_id, info.reciver
                    ),
                };

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    let e = ActorError::NotHelper("network".to_owned());
                    error!(
                        TARGET_DISTRIBUTOR,
                        "GetLastSn, Can not obtain network helper"
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                if let Err(e) = helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::TransferRes { res },
                        },
                    })
                    .await
                {
                    error!(
                        TARGET_DISTRIBUTOR,
                        "GetLastSn, can not send response to network: {}", e
                    );
                    return Err(emit_fail(ctx, e).await);
                };
            }
            DistributorMessage::GetLastSn { subject_id, info } => {
                let metadata = match get_metadata(ctx, &subject_id).await {
                    Ok(metadata) => metadata,
                    Err(e) => {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "GetLastSn, Can not get metadata: {}", e
                        );
                        if let ActorError::NotFound(_) = e {
                            return Err(e);
                        } else {
                            return Err(emit_fail(ctx, e).await);
                        }
                    }
                };

                let gov = match get_gov(ctx, &subject_id).await {
                    Ok(gov) => gov,
                    Err(e) => {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "GetLastSn, Can not get governance: {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };
                let is_owner = if let Some(new_owner) = metadata.new_owner {
                    metadata.owner == info.sender || new_owner == info.sender
                } else {
                    metadata.owner == info.sender
                };

                if !is_owner
                    && !gov.has_this_role(
                        &info.sender.to_string(),
                        Roles::WITNESS,
                        &metadata.schema_id.clone(),
                        metadata.namespace.clone(),
                    )
                {
                    let e = "Sender neither the owned nor a witness";
                    error!(TARGET_DISTRIBUTOR, "GetLastSn, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                }

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    version: info.version,
                    reciver_actor: format!(
                        "/user/node/auth/{}/{}",
                        subject_id, info.reciver
                    )
                };

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    let e = ActorError::NotHelper("network".to_owned());
                    error!(
                        TARGET_DISTRIBUTOR,
                        "GetLastSn, Can not obtain network helper"
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                if let Err(e) = helper
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
                    error!(
                        TARGET_DISTRIBUTOR,
                        "GetLastSn, can not send response to network: {}", e
                    );
                    return Err(emit_fail(ctx, e).await);
                };
            }
            DistributorMessage::SendDistribution {
                actual_sn,
                info,
                gov_version,
                subject_id,
            } => {
                if let Err(e) = self
                    .check_gov_version(ctx, &subject_id, gov_version, &info)
                    .await
                {
                    error!(
                        TARGET_DISTRIBUTOR,
                        "SendDistribution, Can not check governance version {}",
                        e
                    );
                    if let ActorError::Functional(_) = e {
                        return Err(e);
                    } else {
                        return Err(emit_fail(ctx, e).await);
                    };
                }

                let sn = actual_sn.unwrap_or_default();

                // Sacar eventos.
                let (
                    ledger,
                    last_event,
                    last_proof,
                    prev_event_validation_response,
                ) = match self.get_ledger(ctx, &subject_id, sn).await {
                    Ok(res) => res,
                    Err(e) => {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "SendDistribution, Can not obtain ledger {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                let metadata = match get_metadata(ctx, &subject_id).await {
                    Ok(metadata) => metadata,
                    Err(e) => {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "SendDistribution, Can not obtain metadata {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver,
                    request_id: info.request_id,
                    version: info.version,
                    reciver_actor: format!(
                        "/user/node/{}/distributor",
                        subject_id
                    ),
                };

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    let e = ActorError::NotHelper("network".to_owned());
                    error!(
                        TARGET_DISTRIBUTOR,
                        "SendDistribution, Can not obtain network helper {}", e
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                if let Err(e) = helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::DistributionLedgerRes {
                                ledger,
                                last_event,
                                last_proof,
                                prev_event_validation_response,
                                schema: metadata.schema_id,
                                namespace: metadata.namespace.to_string(),
                                governance_id: metadata.governance_id
                            },
                        },
                    })
                    .await
                {
                    error!(
                        TARGET_DISTRIBUTOR,
                        "SendDistribution, can not send response to network: {}",
                        e
                    );
                    return Err(emit_fail(ctx, e).await);
                };
            }
            DistributorMessage::NetworkDistribution {
                request_id,
                event,
                node_key,
                our_key,
                ledger,
                last_proof,
                prev_event_validation_response,
            } => {
                let reciver_actor = format!(
                    "/user/node/{}/distributor",
                    event.content.subject_id
                );

                let message = NetworkMessage {
                    info: ComunicateInfo {
                        request_id,
                        version: 0,
                        sender: our_key,
                        reciver: node_key,
                        reciver_actor,
                    },
                    message: ActorMessage::DistributionLastEventReq {
                        ledger,
                        event,
                        last_proof,
                        prev_event_validation_response,
                    },
                };

                let target = RetryNetwork::default();

                let strategy = Strategy::FixedInterval(
                    FixedIntervalStrategy::new(2, Duration::from_secs(3)),
                );

                let retry_actor = RetryActor::new(target, message, strategy);

                let retry = match ctx
                    .create_child::<RetryActor<RetryNetwork>>(
                        "retry",
                        retry_actor,
                    )
                    .await
                {
                    Ok(retry) => retry,
                    Err(e) => {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "NetworkDistribution, can not create retry actor: {}",
                            e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                if let Err(e) = retry.tell(RetryMessage::Retry).await {
                    error!(
                        TARGET_DISTRIBUTOR,
                        "NetworkDistribution, can not send retry message to retry actor: {}",
                        e
                    );
                    return Err(emit_fail(ctx, e).await);
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
                            error!(
                                TARGET_DISTRIBUTOR,
                                "NetworkResponse, can not send response to distribution actor: {}",
                                e
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
                    } else {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "NetworkResponse, can not obtain distribution actor"
                        );
                        let e = ActorError::NotFound(distribution_path);
                        return Err(emit_fail(ctx, e).await);
                    }

                    'retry: {
                        let Some(retry) = ctx
                            .get_child::<RetryActor<RetryNetwork>>("retry")
                            .await
                        else {
                            // Aquí me da igual, porque al parar este actor para el hijo
                            break 'retry;
                        };

                        if let Err(e) = retry.tell(RetryMessage::End).await {
                            error!(
                                TARGET_DISTRIBUTOR,
                                "NetworkResponse, can not end retry actor: {}",
                                e
                            );
                            // Aquí me da igual, porque al parar este actor para el hijo
                            break 'retry;
                        };
                    }

                    ctx.stop().await;
                }
            }
            DistributorMessage::LastEventDistribution {
                event,
                ledger,
                info,
                last_proof,
                prev_event_validation_response,
            } => {
                if let Err(e) = self
                    .check_auth(
                        ctx,
                        event.signature.signer.clone(),
                        info.clone(),
                        ledger.content.clone(),
                        Some(ledger.content.gov_version),
                        &last_proof.schema_id,
                        last_proof.namespace.clone(),
                        last_proof.governance_id.clone()
                    )
                    .await
                {
                    error!(
                        TARGET_DISTRIBUTOR,
                        "LastEventDistribution, can not check auth: {}", e
                    );
                    if let ActorError::Functional(_) = e {
                        return Err(e);
                    } else {
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                // Llegados a este punto hemos verificado si es la primera copia o no,
                // Si es una gobernanza y está authorizada o el firmante del evento tiene el rol
                // de creator.

                // Ahora hay que crear el sujeto si no existe sn = 0, o aplicar los eventos
                // verificando los hashes y aplicando el patch.
                if ledger.content.event_request.content.is_create_event() {
                    // Creamos el sujeto.
                    if let Err(e) = self.create_subject(ctx, ledger).await {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "LastEventDistribution, can not crate new subject: {}",
                            e
                        );
                        if let ActorError::Functional(_) = e {
                            return Err(e);
                        } else {
                            return Err(emit_fail(ctx, e).await);
                        }
                    };
                } else {
                    let subject_ref = match self
                        .get_subject(ctx, ledger.content.subject_id.clone())
                        .await
                    {
                        Ok(subject) => subject,
                        Err(e) => {
                            error!(
                                TARGET_DISTRIBUTOR,
                                "LastEventDistribution, can not get SubjectRef: {}",
                                e
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
                    };

                    let response = match subject_ref
                        .ask(SubjectMessage::UpdateLedger {
                            events: vec![ledger.clone()],
                        })
                        .await
                    {
                        Ok(res) => res,
                        Err(e) => {
                            error!(
                                TARGET_DISTRIBUTOR,
                                "LastEventDistribution, can not update ledger: {}",
                                e
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
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
                                    Err(e) => {
                                        error!(
                                            TARGET_DISTRIBUTOR,
                                            "LastEventDistribution, can not obtain governance: {}",
                                            e
                                        );
                                        return Err(emit_fail(ctx, e).await);
                                    }
                                };

                                let our_gov_version = gov.version;

                                let new_info = ComunicateInfo {
                                    reciver: info.sender,
                                    sender: info.reciver,
                                    request_id: info.request_id,
                                    version: info.version,
                                    reciver_actor: format!(
                                        "/user/node/{}/distributor",
                                        ledger.content.subject_id
                                    ),
                                };

                                let helper: Option<Intermediary> =
                                    ctx.system().get_helper("network").await;

                                let Some(mut helper) = helper else {
                                    error!(
                                        TARGET_DISTRIBUTOR,
                                        "LastEventDistribution, can not obtain network helper"
                                    );
                                    let e = ActorError::NotHelper(
                                        "network".to_owned(),
                                    );
                                    return Err(emit_fail(ctx, e).await);
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
                                error!(TARGET_DISTRIBUTOR, "LastEventDistribution, can not send response to network: {}", e);
                                return Err(emit_fail(ctx, e).await);
                            };
                                return Ok(());
                            }
                        }
                        _ => {
                            let e = ActorError::UnexpectedResponse(
                                ActorPath::from(format!(
                                    "/user/node/{}",
                                    ledger.content.subject_id
                                )),
                                "SubjectResponse::LastSn".to_owned(),
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
                    };
                }

                if let Err(e) = update_event(ctx, event.clone()).await {
                    if let ActorError::Functional(_) = e {
                        warn!(
                            TARGET_DISTRIBUTOR,
                            "LastEventDistribution, can not update event: {}",
                            e
                        );
                    } else {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "LastEventDistribution, can not update event: {}",
                            e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                if let Err(e) = update_vali_data(
                    ctx,
                    last_proof,
                    prev_event_validation_response,
                )
                .await
                {
                    if let ActorError::Functional(_) = e {
                        warn!(
                            TARGET_DISTRIBUTOR,
                            "LastEventDistribution, can not update validation data: {}",
                            e
                        );
                        return Err(e);
                    } else {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "LastEventDistribution, can not update validation data: {}",
                            e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                let objetive = if info.request_id.is_empty() {
                    format!("node/{}/distribution", event.content.subject_id)
                } else {
                    info.request_id.clone()
                };

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    version: info.version,
                    reciver_actor: format!(
                        "/user/{}/{}",
                        objetive,
                        info.reciver.clone()
                    ),
                };

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    error!(
                        TARGET_DISTRIBUTOR,
                        "LastEventDistribution, can not obtain network helper"
                    );
                    let e = ActorError::NotHelper("network".to_owned());
                    return Err(emit_fail(ctx, e).await);
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
                    error!(
                        TARGET_DISTRIBUTOR,
                        "LastEventDistribution, can not send response to network: {}",
                        e
                    );
                    return Err(emit_fail(ctx, e).await);
                };
            }
            DistributorMessage::LedgerDistribution {
                mut events,
                info,
                last_event,
                last_proof,
                prev_event_validation_response,
                namespace,
                schema,
                governance_id
            } => {
                // TODO REFACTORIZAR.
                if events.is_empty() {
                    warn!(
                        TARGET_DISTRIBUTOR,
                        "LedgerDistribution, events is empty"
                    );
                    return Err(ActorError::Functional(
                        "Events is empty".to_owned(),
                    ));
                }

                if let Err(e) = self
                    .check_auth(
                        ctx,
                        events[0].signature.signer.clone(),
                        info.clone(),
                        events[0].content.clone(),
                        None,
                        &schema,
                        namespace,
                        governance_id
                    )
                    .await
                {
                    error!(
                        TARGET_DISTRIBUTOR,
                        "LedgerDistribution, can not check auth: {}", e
                    );
                    if let ActorError::Functional(_) = e {
                        return Err(e);
                    } else {
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                let subject_id = events[0].content.subject_id.clone();

                if events[0].content.event_request.content.is_create_event() {
                    // Creamos el sujeto.
                    if let Err(e) =
                        self.create_subject(ctx, events[0].clone()).await
                    {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "LedgerDistribution, can not create new subject: {}",
                            e
                        );
                        if let ActorError::Functional(_) = e {
                            return Err(e);
                        } else {
                            return Err(emit_fail(ctx, e).await);
                        }
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
                    Err(e) => {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "LedgerDistribution, can not obtain SubjectRef: {}",
                            e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                // Obtenemos el last_sn para saber si nos vale la pena intentar actualizar el ledger
                let response =
                    match subject_ref.ask(SubjectMessage::GetMetadata).await {
                        Ok(res) => res,
                        Err(e) => return Err(emit_fail(ctx, e).await),
                    };

                let metadata = match response {
                    SubjectResponse::Metadata(data) => data,
                    _ => {
                        let e = ActorError::UnexpectedResponse(
                            ActorPath::from(format!(
                                "/user/node/{}",
                                subject_id
                            )),
                            "SubjectResponse::Metadata".to_owned(),
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                let last_sn_events = if let Some(last_ledger) = events.last() {
                    last_ledger.content.sn
                } else {
                    unreachable!();
                };

                let last_sn = if last_sn_events > metadata.sn {
                    let response = match subject_ref
                        .ask(SubjectMessage::UpdateLedger { events })
                        .await
                    {
                        Ok(res) => res,
                        Err(e) => return Err(emit_fail(ctx, e).await),
                    };

                    match response {
                        SubjectResponse::LastSn(last_sn) => last_sn,
                        _ => {
                            let e = ActorError::UnexpectedResponse(
                                ActorPath::from(format!(
                                    "/user/node/{}",
                                    subject_id
                                )),
                                "SubjectResponse::LastSn".to_owned(),
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
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
                            // No apliqué todos los eventos, no quiero su event.
                        }
                        std::cmp::Ordering::Equal => {
                            // Quiere decir que apliqué todos los eventos y todo fue bien.
                            if let Err(e) = update_event(ctx, event).await {
                                if let ActorError::Functional(_) = e {
                                    warn!(
                                        TARGET_DISTRIBUTOR,
                                        "LedgerDistribution, can not update event: {}",
                                        e
                                    );
                                    return Err(e);
                                } else {
                                    error!(
                                        TARGET_DISTRIBUTOR,
                                        "LedgerDistribution, can not update event: {}",
                                        e
                                    );
                                    return Err(emit_fail(ctx, e).await);
                                }
                            };

                            let Some(last_proof) = last_proof else {
                                let e = format!(
                                    "last proof is empty, can not update for {}",
                                    subject_id
                                );
                                error!(
                                    TARGET_DISTRIBUTOR,
                                    "LedgerDistribution, {}", e
                                );
                                return Err(ActorError::Functional(e));
                            };

                            let Some(prev_event_validation_response) =
                                prev_event_validation_response
                            else {
                                let e = format!(
                                    "prev event validation response is empty, can not update for {}",
                                    subject_id
                                );
                                error!(
                                    TARGET_DISTRIBUTOR,
                                    "LedgerDistribution, {}", e
                                );
                                return Err(ActorError::Functional(e));
                            };

                            if let Err(e) = update_vali_data(
                                ctx,
                                last_proof,
                                prev_event_validation_response,
                            )
                            .await
                            {
                                if let ActorError::Functional(_) = e {
                                    warn!(
                                        TARGET_DISTRIBUTOR,
                                        "LedgerDistribution, can not update validation data: {}",
                                        e
                                    );
                                    return Err(e);
                                } else {
                                    error!(
                                        TARGET_DISTRIBUTOR,
                                        "LedgerDistribution, can not update validation data: {}",
                                        e
                                    );
                                    return Err(emit_fail(ctx, e).await);
                                }
                            };
                        }
                        std::cmp::Ordering::Greater => {
                            let our_path = ActorPath::from(format!(
                                "/user/node/{}/distributor",
                                subject_id
                            ));
                            let our_actor: Option<ActorRef<Distributor>> =
                                ctx.system().get_actor(&our_path).await;
                            if let Some(our_actor) = our_actor {
                                if let Err(e) = our_actor
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
                                    return Err(emit_fail(ctx, e).await);
                                }
                            } else {
                                let e = ActorError::NotFound(our_path);
                                return Err(emit_fail(ctx, e).await);
                            }
                        }
                    };

                    return Ok(());
                };

                let gov_id = if metadata.schema_id == "governance" {
                    metadata.subject_id
                } else {
                    metadata.governance_id
                };

                let gov_version = match get_gov(ctx, &gov_id.to_string()).await
                {
                    Ok(gov) => gov.version,
                    Err(e) => {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "LedgerDistribution, can not obtain governance: {}",
                            e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    version: info.version,
                    reciver_actor: format!(
                        "/user/node/{}/distributor",
                        subject_id
                    ),
                };

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    error!(
                        TARGET_DISTRIBUTOR,
                        "LedgerDistribution, can not obtain netowrk helper"
                    );
                    let e = ActorError::NotHelper("network".to_owned());
                    return Err(emit_fail(ctx, e).await);
                };

                if let Err(e) = helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::DistributionLedgerReq {
                                gov_version: Some(gov_version),
                                actual_sn: Some(last_sn),
                                subject_id,
                            },
                        },
                    })
                    .await
                {
                    error!(
                        TARGET_DISTRIBUTOR,
                        "LedgerDistribution, can not send response to network: {}",
                        e
                    );
                    return Err(emit_fail(ctx, e).await);
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
                    if let Err(e) = distribuiton_actor
                        .tell(DistributionMessage::Response {
                            sender: self.node.clone(),
                        })
                        .await
                    {
                        error!(
                            TARGET_DISTRIBUTOR,
                            "OnChildError, can not send response to Distribution actor: {}",
                            e
                        );
                        emit_fail(ctx, e).await;
                    }
                } else {
                    let e = ActorError::NotFound(distribuiton_path);
                    error!(
                        TARGET_DISTRIBUTOR,
                        "OnChildError, can not obtain Distribution actor: {}",
                        e
                    );
                    emit_fail(ctx, e).await;
                }
                ctx.stop().await;
            }
            _ => {
                error!(TARGET_DISTRIBUTOR, "OnChildError, unexpected error");
            }
        };
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Distributor>,
    ) -> ChildAction {
        error!(TARGET_DISTRIBUTOR, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}
