//

mod config;
mod error;
mod manager;
mod service;

pub use config::Config;
pub use error::Error;




use identity::{keys::KeyPair, identifier::derive::KeyDerivator};
use network::NetworkWorker;
use kore_common::NetworkMessage;
use rush::{ActorSystem, SystemRef, SqliteCollection, SqliteManager};

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use prometheus_client::registry::Registry;

/// The Kore Client API.
/// 
pub struct Api {
    /// The network configuration.
    pub config: Config,

}

impl Api {
    /// Create a new API instance.
    pub async fn build(
        keys: KeyPair,
        config: Config,
        registry: &mut Registry,
        cancellation_token: CancellationToken,
    ) -> Result<Self, Error> {

        // Create the actor system.
        let (system, system_runner) = system(cancellation_token.clone(), &config.db_path)
            .await
            .expect("Failed to create actor system");

        // Create the network worker.
        let network_worker = NetworkWorker::<NetworkMessage>::new(
            registry,
            keys,
            config.network_config.clone(),
            None,
            KeyDerivator::Ed25519,
            cancellation_token.clone()
        ).map_err(|e| Error::Network(format!("Can't create network worker: {}", e)));

        Ok(Self { config })
    }
}

/// Create the actor system for the Kore Client. 
async fn system(cancellation_token: CancellationToken, db_path: &str) -> Result<(SystemRef, JoinHandle<()>), Error> {
    // Create the actor system.
    let (mut system_ref, mut system_runner) = ActorSystem::create(cancellation_token);

    // Add the database helper.
    let db = SqliteManager::new(db_path)
        .map_err(|e| Error::Init(format!("Failed to create DB manager: {}", e)))?;
    system_ref.add_helper("store", db).await;

    // Run the system.
    let runner = tokio::spawn(async move {
        system_runner.run().await;
    });

    Ok((system_ref, runner))
}