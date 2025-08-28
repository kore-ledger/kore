use std::{net::SocketAddr, path::PathBuf};

use axum::{
    BoxError,
    handler::HandlerWithoutStateExt,
    http::{Method, StatusCode, Uri, header},
    response::Redirect,
};
use axum_extra::extract::Host;
use axum_server::{Handle, tls_rustls::RustlsConfig};
use enviroment::{
    build_address_http, build_address_https, build_https_cert,
    build_https_private_key,
};
use kore_bridge::{
    Bridge,
    clap::Parser,
    settings::{build_config, build_file_path, build_password, command::Args},
};
use middleware::tower_trace;
use server::build_routes;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

mod enviroment;
mod error;
mod logging;
mod middleware;
mod server;
mod wrappers;

mod doc;
#[derive(Clone)]
struct Ports {
    http: String,
    https: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let mut password = args.password;
    if password.is_empty() {
        password = build_password();
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
    logging::init_logging(&config.logging);
    let bridge = Bridge::build(config, &password, None).await.unwrap();
    let token = bridge.token().clone();

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
            tokio::select! {
                _ = token.cancelled() => {
                    handle.graceful_shutdown(None);
                }
            }
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
            tokio::select! {
                _ = token.cancelled() => {
                }
            }
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

        parts.scheme = Some(axum::http::uri::Scheme::HTTPS);

        if parts.path_and_query.is_none() {
            parts.path_and_query = Some("/".parse().unwrap());
        }

        let https_host =
            host.replace(&ports.http.to_string(), &ports.https.to_string());
        parts.authority = Some(https_host.parse()?);

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
