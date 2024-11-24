use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Handler, SystemEvent,
};
use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};

use crate::{intermediary::Intermediary, NetworkMessage};

use super::{common::emit_fail, TimeStamp};

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
        __sender: ActorPath,
        msg: NetworkMessage,
        ctx: &mut ActorContext<RetryNetwork>,
    ) -> Result<(), ActorError> {
        let helper: Option<Intermediary> =
            ctx.system().get_helper("network").await;

        let Some(mut helper) = helper else {
            let e = ActorError::NotHelper("network".to_owned());
            return Err(emit_fail(ctx, e).await);
        };

        if let Err(_e) = helper
            .send_command(network::CommandHelper::SendMessage { message: msg })
            .await
        {
            // error al enviar mensaje, propagar hacia arriba TODO
        };
        Ok(())
    }
}
