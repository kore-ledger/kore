use std::env;

use config::Config;
use params::Params;

mod command;
use crate::config::Config as BridgeConfig;
mod params;

pub fn build_config(env: bool, file: &str) -> BridgeConfig {
    // Env configuration
    let mut params_env = Params::default();
    if env {
        params_env = Params::from_env();
    }

    // file configuration (json, yaml or toml)
    let mut params_file = Params::default();
    if !file.is_empty() {
        let mut config = Config::builder();

        config = config.add_source(config::File::with_name(file));

        let config = config
            .build()
            .map_err(|e| {
                println!("Error building config: {}", e);
            })
            .unwrap();

        params_file = config
            .try_deserialize()
            .map_err(|e| {
                println!("Error try deserialize config: {}", e);
            })
            .unwrap();
    }

    // Mix configurations.
    BridgeConfig::from(params_env.mix_config(params_file))
}

pub fn build_password() -> String {
    env::var("KORE_PASSWORD").unwrap()
}

pub fn build_file_path() -> String {
    env::var("KORE_FILE_PATH").unwrap_or_default()
}