use actor::{Actor, ActorContext, ActorPath, Error as ActorError, Handler};
use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};

use crate::{intermediary::Intermediary, NetworkMessage};

use super::TimeStamp;

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
            ctx.system().get_helper("NetworkIntermediary").await;
        let mut helper = if let Some(helper) = helper {
            helper
        } else {
            // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
            // return Err(ActorError::Get("Error".to_owned()))
            return Err(ActorError::NotHelper);
        };

        if let Err(e) = helper
            .send_command(network::CommandHelper::SendMessage { message: msg })
            .await
        {
            // error al enviar mensaje, propagar hacia arriba TODO
        };
        Ok(())
    }
}
