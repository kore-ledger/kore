// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use axum::{response::IntoResponse, routing::get, Extension, Router};
use prometheus_client::{encoding::text::encode, registry::Registry};

pub async fn handler_prometheus_data(
    Extension(state): Extension<Arc<Registry>>,
) -> impl IntoResponse {
    let mut body = String::new();
    if let Err(e) = encode(&mut body, &state) {
        return (
            [("Content-Type", "text/plain; charset=utf-8")],
            format!("Error encoding Prometheus metrics: {}", e),
        );
    };

    ([("Content-Type", "text/plain; charset=utf-8")], body)
}

pub fn build_routes(registry: Registry) -> Router {
    let state = Arc::new(registry);

    let endpoints = Router::new()
        .route("/metrics", get(handler_prometheus_data))
        .layer(Extension(state));

    Router::new().merge(endpoints)
}

pub fn run_prometheus(registry: Registry, tcp_listener: &str) {
    let routes = build_routes(registry);
    let tcp_listener = tcp_listener.to_owned();

    tokio::spawn(async move {
        let listener =
            tokio::net::TcpListener::bind(tcp_listener).await.unwrap();
        axum::serve(listener, routes).await.unwrap();
    });
}
