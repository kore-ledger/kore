use file_rotate::compression::Compression;
use file_rotate::TimeFrequency;
use file_rotate::{ContentLimit, FileRotate, suffix::AppendCount};
use kore_bridge::{Logging, LoggingRotation};
use reqwest::Client;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::fmt::{self, writer::BoxMakeWriter};
use tracing_subscriber::{EnvFilter, Registry, prelude::*};

pub struct LoggingHandle {
    _vec: Vec<WorkerGuard>
}

struct ApiEventWriter {
    buf: Vec<u8>,
    tx: Arc<mpsc::Sender<Vec<u8>>>,
}

impl Write for ApiEventWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Acumulamos en memoria para no fragmentar el evento
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.buf.is_empty() {
            // Envío no bloqueante; si el canal está lleno, se descarta
            let _ = self.tx.try_send(std::mem::take(&mut self.buf));
        }
        Ok(())
    }
}

impl Drop for ApiEventWriter {
    fn drop(&mut self) {
        // Asegura que el evento se empuje aunque no llamen a flush()
        let _ = self.flush();
    }
}

pub async fn init_logging(cfg: &Logging) -> Option<LoggingHandle> {
    if !cfg.logs() {
        return None;
    }

    let Logging {
        output,
        api_url,
        file_path,
        rotation,
        max_size,
        max_files,
    } = cfg.clone();

    let mut guards: Vec<WorkerGuard> = Vec::new();

    let env_filter = if let Ok(env_filter) = EnvFilter::try_from_default_env() {
        env_filter
    } else {
        EnvFilter::new("info")
    };

    let stdout_layer = output.stdout.then(|| {
        let (stdout_nb, guard) = NonBlocking::new(io::stdout());
        guards.push(guard);

        let mw = {
            let nb = stdout_nb.clone();
            BoxMakeWriter::new(move || -> Box<dyn Write + Send + Sync> {
                Box::new(nb.clone())
            })
        };

        fmt::layer()
            .with_target(true)
            .with_ansi(true)
            .with_writer(mw)
    });

    let file_layer = output.file.then(|| {
        std::fs::create_dir_all(&file_path).ok();

        let limit = match rotation {
            LoggingRotation::Size => ContentLimit::Bytes(max_size),
            LoggingRotation::Hourly => {
                ContentLimit::Time(TimeFrequency::Hourly)
            }
            LoggingRotation::Daily => {
                ContentLimit::Time(TimeFrequency::Daily)
            }
            LoggingRotation::Weekly => {
                ContentLimit::Time(TimeFrequency::Weekly)
            }
            LoggingRotation::Monthly => {
                ContentLimit::Time(TimeFrequency::Monthly)
            }
            LoggingRotation::Yearly => {
                ContentLimit::Time(TimeFrequency::Yearly)
            }
            LoggingRotation::Never => ContentLimit::None,
        };

        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true).append(true);

        let full = format!("{}/kore.log", file_path);
        let fr = FileRotate::new(
            &full,
            AppendCount::new(max_files),
            limit,
            Compression::None,
            Some(opts),
        );

        let (file_nb, guard) = NonBlocking::new(fr);
        guards.push(guard);

        let mw = {
            let nb = file_nb.clone();
            BoxMakeWriter::new(move || -> Box<dyn Write + Send + Sync> {
                Box::new(nb.clone())
            })
        };

        fmt::layer()
            .with_target(true)
            .with_ansi(false)
            .with_writer(mw)
    });

    let mut api_rx: Option<mpsc::Receiver<Vec<u8>>> = None;
    let mut api_url_final: Option<String> = None;

    let api_layer = (output.api && api_url.is_some()).then(|| {
        let (tx, rx) = mpsc::channel::<Vec<u8>>(10_000);
        api_rx = Some(rx);
        api_url_final = api_url.clone();

        let tx = Arc::new(tx);
        let mw = {
            let tx = tx.clone();
            BoxMakeWriter::new(move || -> Box<dyn Write + Send + Sync> {
                Box::new(ApiEventWriter {
                    buf: Vec::with_capacity(512),
                    tx: tx.clone(),
                })
            })
        };

        fmt::layer()
            .with_target(true)
            .with_ansi(false)
            .with_writer(mw)
    });

    let subscriber = Registry::default()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .with(api_layer);

    // If a subscriber is running (e.g. tests)
    if subscriber.try_init().is_err() {
        return None;
    }

    if let (Some(mut rx), Some(url)) = (api_rx, api_url_final) {
        tokio::spawn(async move {
            let client = Client::new();
            while let Some(bytes) = rx.recv().await {
                let _ = client.post(&url).body(bytes).send().await;
            }
        });
    }

    Some(LoggingHandle{_vec: guards})
}
