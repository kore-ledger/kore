// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{Actor, ActorContext, ActorPath, Error as ActorError, Handler};
use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{NetworkMessage, intermediary::Intermediary};

use super::{TimeStamp, common::emit_fail};

const TARGET_NETWORK: &str = "Kore-Model-Network";

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
    Ord,
)]
pub struct TimeOutResponse {
    pub who: KeyIdentifier,
    pub re_trys: u32,
    pub timestamp: TimeStamp,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct RetryNetwork {}

#[async_trait]
impl Actor for RetryNetwork {
    type Event = ();
    type Message = NetworkMessage;
    type Response = ();
}

#[async_trait]
impl Handler<RetryNetwork> for RetryNetwork {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: NetworkMessage,
        ctx: &mut ActorContext<RetryNetwork>,
    ) -> Result<(), ActorError> {
        let helper: Option<Intermediary> =
            ctx.system().get_helper("network").await;

        let Some(mut helper) = helper else {
            let e = ActorError::NotHelper("network".to_owned());
            error!(TARGET_NETWORK, "Can not obtain network helper");
            return Err(emit_fail(ctx, e).await);
        };

        if let Err(e) = helper
            .send_command(network::CommandHelper::SendMessage { message: msg })
            .await
        {
            error!(TARGET_NETWORK, "Can not send message to network helper");
            return Err(emit_fail(ctx, e).await);
        };
        Ok(())
    }
}
