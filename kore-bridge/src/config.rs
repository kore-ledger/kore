use kore_base::config::Config as KoreConfig;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    /// Settings from Kore Base.
    pub kore_config: KoreConfig,
    /// Path for encryptep keys.
    #[serde(rename = "keysPath")]
    pub keys_path: String,
    /// TcpListener from prometheus axum server.
    pub prometheus: String,
}