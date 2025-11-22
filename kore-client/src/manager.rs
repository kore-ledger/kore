//

use rush::{Actor, ActorContext, ActorError, ActorPath, Event, Handler, Message, PersistentActor,
     Response, SqliteManager, FullPersistence};

use serde::{Deserialize, Serialize};
use async_trait::async_trait;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientManager {
    // Add fields as necessary
}

impl ClientManager {
    /// Create a new ClientManager instance.
    pub fn new() -> Self {
        Self {
            // Initialize fields as necessary
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ManagerMessasge {
    // Define manager messages as necessary
}

impl Message for ManagerMessasge {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ManagerResponse {
    // Define manager responses as necessary
    Ok,
}

impl Response for ManagerResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerEvent {
    // Define manager events as necessary
}

impl Event for ManagerEvent {}

#[async_trait]
impl Actor for ClientManager {
    type Message = ManagerMessasge;
    type Response = ManagerResponse;
    type Event = ManagerEvent;

    async fn pre_start(&mut self, ctx: &mut ActorContext<Self>) -> Result<(), ActorError> {
        // Initialization logic here
        let db = match ctx.system().get_helper::<SqliteManager>("store").await {
            Some(db) => db,
            None => {
                return Err(ActorError::CreateStore(
                    "Database not found".to_string(),
                ));
            }
        };
        Ok(())
    }

    async fn post_stop(&mut self, _ctx: &mut ActorContext<Self>) -> Result<(), ActorError> {
        // Cleanup logic here
        Ok(())
    }
}

#[async_trait]
impl Handler<ClientManager> for ClientManager {

    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        _msg: ManagerMessasge,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<ManagerResponse, ActorError> {
        // Handle messages here
        Ok(ManagerResponse::Ok )
    }
}

#[async_trait]
impl PersistentActor for ClientManager {
    type Persistence = FullPersistence;

    fn apply(&mut self, _event: &ManagerEvent) -> Result<(), ActorError> {
        // Apply events here
        Ok(())
    }
}

