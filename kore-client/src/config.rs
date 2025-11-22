//

use network::Config as NetworkConfig;
use serde::Deserialize;

/// The Kore Client configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// The database path.
    pub db_path: String,
    /// The network configuration.
    pub network_config: NetworkConfig,
}