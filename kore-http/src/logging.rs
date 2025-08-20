use file_rotate::compression::Compression;
use kore_bridge::Logging;
use std::fs::OpenOptions;
use std::io::{self, sink, Write};
use std::path::{PathBuf};
use tracing_appender::non_blocking::NonBlocking;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use file_rotate::{FileRotate, suffix::AppendCount, ContentLimit};
use tracing_subscriber::{fmt, EnvFilter, prelude::*, fmt::writer::BoxMakeWriter};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use reqwest::Client;
pub struct AsyncApiWriter {
    tx: UnboundedSender<String>,
}

impl AsyncApiWriter {
    pub fn new(url: String) -> Self {
        // Creamos el canal
        let (tx, mut rx) = unbounded_channel::<String>();
        // Cliente reqwest reutilizable
        let client = Client::new();
        // Spawn de fondo: consume el canal y hace POST
        tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                let _ = client.post(&url)
                    .body(line)
                    .send()
                    .await;
            }
        });
        AsyncApiWriter { tx }
    }
}

impl Write for AsyncApiWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Convertimos bytes a string (línea o chunk)
        if let Ok(s) = std::str::from_utf8(buf) {
            let _ = self.tx.send(s.to_string());
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

pub fn init_logging(cfg: &Logging) {
    // 1) Clona por valor todo lo que vayamos a capturar:
    let level        = cfg.level.clone();
    let file_path    = cfg.file_path.clone();
    let rotation     = cfg.rotation.clone();
    let max_files    = cfg.max_files;
    let max_size     = cfg.max_size as usize;
    let api_url      = cfg.api_url.clone();

    // 2) Interpreta OUTPUT en 3 booleans, sin referencias a cfg:
    let parts: Vec<String> = cfg
        .output
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let enable_stdout = parts.iter().any(|p| p == "stdout");
    let enable_file   = parts.iter().any(|p| p == "file");
    let enable_api    = parts.iter().any(|p| p == "api");

    // 3) Capa stdout
    let stdout_layer = fmt::layer()
        .with_target(true)
        .with_writer(BoxMakeWriter::new(move || {
            if enable_stdout {
                Box::new(io::stdout()) as Box<dyn Write + Send + Sync>
            } else {
                Box::new(sink())
            }
        }))
        .with_filter(EnvFilter::new(&level));

    // 4) Capa fichero
    let file_layer = fmt::layer()
    .with_target(true)
    .with_writer(BoxMakeWriter::new(move || {
        if enable_file {
            let writer: Box<dyn Write + Send + Sync> = match rotation.as_str() {
                "size" => {
                    let mut opts = OpenOptions::new();
                    opts.read(true).write(true).create(true).append(true);
                    Box::new(FileRotate::new(
                        &file_path,
                        AppendCount::new(max_files),
                        ContentLimit::Bytes(max_size),
                        Compression::None,
                        Some(opts),
                    )) as Box<dyn Write + Send + Sync>
                }
                "hourly" | "daily" | "never" => {
                    let when = match rotation.as_str() {
                        "hourly" => Rotation::HOURLY,
                        "daily" => Rotation::DAILY,
                        "never" => Rotation::NEVER,
                        _ => unreachable!(),
                    };
                    let app = RollingFileAppender::new(
                        when,
                        PathBuf::from(&file_path).parent().unwrap(),
                        PathBuf::from(&file_path)
                            .file_name().unwrap()
                            .to_str().unwrap(),
                    );
                    let (nb, _guard) = NonBlocking::new(app);
                    Box::new(nb) as Box<dyn Write + Send + Sync>
                }
                _ => Box::new(sink()) as Box<dyn Write + Send + Sync>,
            };
            writer
        } else {
            Box::new(sink()) as Box<dyn Write + Send + Sync>
        }
    }))
    .with_filter(EnvFilter::new(&level));


    // 5) Capa API
    let api_layer = fmt::layer()
        .with_target(true)
        .with_writer(BoxMakeWriter::new(move || {
            if enable_api {
                if let Some(url) = api_url.clone() {
                    Box::new(AsyncApiWriter::new(url)) as Box<dyn Write + Send + Sync>
                } else {
                    Box::new(sink())
                }
            } else {
                Box::new(sink())
            }
        }))
        .with_filter(EnvFilter::new(&level));

    // 6) Monta todo el subscriber
    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .with(api_layer)
        .init();
}
