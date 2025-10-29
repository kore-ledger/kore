use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use rush::{
    Actor, ActorContext, ActorError, ActorPath, Handler, Message, Response,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeKey {
    key: KeyIdentifier,
}

impl NodeKey {
    pub fn new(key: KeyIdentifier) -> Self {
        Self { key }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeKeyMessage {
    GetKeyIdentifier,
}

impl Message for NodeKeyMessage {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeKeyResponse {
    KeyIdentifier(KeyIdentifier),
}

impl Response for NodeKeyResponse {}

#[async_trait]
impl Actor for NodeKey {
    type Message = NodeKeyMessage;
    type Event = ();
    type Response = NodeKeyResponse;

    async fn pre_start(
        &mut self,
        _ctx: &mut rush::ActorContext<Self>,
    ) -> Result<(), ActorError> {
        Ok(())
    }

    async fn pre_stop(
        &mut self,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        Ok(())
    }
}

#[async_trait]
impl Handler<NodeKey> for NodeKey {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: NodeKeyMessage,
        _ctx: &mut rush::ActorContext<NodeKey>,
    ) -> Result<NodeKeyResponse, ActorError> {
        match msg {
            NodeKeyMessage::GetKeyIdentifier => {
                Ok(NodeKeyResponse::KeyIdentifier(self.key.clone()))
            }
        }
    }
}
