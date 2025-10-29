use clap::{Parser, command};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to the file containing the settings you want to use
    #[arg(short, long, default_value_t = String::default())]
    pub file_path: String,

    /// Bulean to indicate whether you want to use the environment variables as a configuration (file_path compatible)
    #[arg(short, long, default_value_t = true)]
    pub env_config: bool,

    /// Password to be used for the creation of the cryptographic material, if not specified, the password of the environment variable 'KORE_PASSWORD' will be used.
    #[arg(short, long, default_value_t = String::default())]
    pub password: String,

    /// Password to be used to auth for sinks.
    #[arg(short = 's', long, default_value_t = String::default())]
    pub password_sink: String,
}
