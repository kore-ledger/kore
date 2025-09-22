use std::{net::SocketAddr, path::PathBuf, time::Duration};

use axum::{
    BoxError,
    handler::HandlerWithoutStateExt,
    http::{
        Method, StatusCode, Uri, header,
        uri::{Authority, Scheme},
    },
    response::Redirect,
};
use axum_extra::extract::Host;
use axum_server::{Handle, tls_rustls::RustlsConfig};
use enviroment::{
    build_address_http, build_address_https, build_https_cert,
    build_https_private_key,
};
use futures::future::join_all;
use kore_bridge::{
    Bridge,
    clap::Parser,
    settings::{
        build_config, build_file_path, build_password, build_sink_password,
        command::Args,
    },
};
use middleware::tower_trace;
use server::build_routes;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

mod enviroment;
mod error;
mod logging;
mod middleware;
mod server;
mod wrappers;

mod doc;

#[cfg(all(feature = "sqlite", feature = "rocksdb"))]
compile_error!("Select only one: 'sqlite' or 'rocksdb'.");

#[cfg(not(any(feature = "sqlite", feature = "rocksdb")))]
compile_error!("You must enable 'sqlite' or 'rocksdb'.");

#[cfg(not(feature = "ext-sqlite"))]
compile_error!("You must enable 'ext-sqlite'.");

#[derive(Clone)]
struct Ports {
    http: String,
    https: String,
}

const TARGET_HTTP: &str = "KoreHttp";

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let mut password = args.password;
    if password.is_empty() {
        password = build_password();
    }

    let mut password_sink = args.password_sink;
    if password_sink.is_empty() {
        password_sink = build_sink_password();
    }

    let mut file_path = args.file_path;
    if file_path.is_empty() {
        file_path = build_file_path();
    }

    let https_address = build_address_https();

    let listener_http = tokio::net::TcpListener::bind(build_address_http())
        .await
        .unwrap();

    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([header::CONTENT_TYPE])
        .allow_origin(Any);

    let config = build_config(args.env_config, &file_path).unwrap();
    let _log_handle = logging::init_logging(&config.logging).await;

    let (bridge, runners) =
        Bridge::build(config, &password, &password_sink, None)
            .await
            .unwrap();

    if !https_address.is_empty() {
        let https_address = https_address.parse::<SocketAddr>().unwrap();

        tokio::spawn(redirect_http_to_https(
            https_address.port(),
            listener_http,
        ));
        rustls::crypto::ring::default_provider()
            .install_default()
            .unwrap();

        let tls = RustlsConfig::from_pem_file(
            PathBuf::from(&build_https_cert()),
            PathBuf::from(&build_https_private_key()),
        )
        .await
        .unwrap();

        let handle = Handle::new();

        let handle_clone = handle.clone();
        tokio::spawn(async move {
            join_all(runners).await;
            handle.graceful_shutdown(Some(Duration::from_secs(10)));
            info!(TARGET_HTTP, "All the runners have stopped");
        });

        axum_server::bind_rustls(https_address, tls)
            .handle(handle_clone)
            .serve(
                tower_trace(build_routes(bridge))
                    .layer(cors)
                    .into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
    } else {
        axum::serve(
            listener_http,
            tower_trace(build_routes(bridge))
                .layer(cors)
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            join_all(runners).await;
            info!(TARGET_HTTP, "All the runners have stopped");
        })
        .await
        .unwrap()
    }
}

async fn redirect_http_to_https(https: u16, listener_http: TcpListener) {
    fn make_https(
        host: String,
        uri: Uri,
        ports: Ports,
    ) -> Result<Uri, BoxError> {
        let mut parts = uri.into_parts();
        parts.scheme = Some(Scheme::HTTPS);

        if parts.path_and_query.is_none() {
            parts.path_and_query = Some("/".parse()?);
        }

        let auth: Authority = host.parse()?;

        let http_port: u16 = ports.http.parse()?;
        let https_port: u16 = ports.https.parse()?;

        let new_auth_str = match auth.port() {
            Some(p) if p == http_port => {
                format!("{}:{}", auth.host(), https_port)
            }
            Some(_) => auth.as_str().to_string(), // puerto “no esperado”: no lo tocamos
            None => {
                if https_port == 443 {
                    auth.host().to_string()
                } else {
                    format!("{}:{}", auth.host(), https_port)
                }
            }
        };

        parts.authority = Some(new_auth_str.parse()?);
        Ok(Uri::from_parts(parts)?)
    }

    let ports = Ports {
        https: https.to_string(),
        http: listener_http.local_addr().unwrap().port().to_string(),
    };

    let redirect = move |Host(host): Host, uri: Uri| async move {
        match make_https(host, uri, ports) {
            Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
            Err(error) => {
                tracing::warn!(%error, "failed to convert URI to HTTPS");
                Err(StatusCode::BAD_REQUEST)
            }
        }
    };

    axum::serve(listener_http, redirect.into_make_service())
        .await
        .unwrap();
}
