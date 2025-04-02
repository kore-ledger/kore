use std::time::Duration;
use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
use tracing::{Span, debug, error, info_span};

use axum::{
    Router,
    body::Bytes,
    extract::{MatchedPath, Request},
    http::HeaderMap,
    response::Response,
};

pub fn tower_trace(routes: Router) -> Router {
    routes.layer(
        TraceLayer::new_for_http()
            .make_span_with(|request: &Request<_>| {
                let matched_path = request
                    .extensions()
                    .get::<MatchedPath>()
                    .map(MatchedPath::as_str);

                info_span!(
                    "http_request",
                    method = ?request.method(),
                    matched_path,
                    some_other_field = tracing::field::Empty,
                )
            })
            .on_request(|request: &Request<_>, _span: &Span| {
                debug!("New request: {} {}", request.method(), request.uri().path())
            })
            .on_response(|_response: &Response, latency: Duration, _span: &Span| {
                debug!("Response generated in {:?}", latency)
            })
            .on_body_chunk(|chunk: &Bytes, _latency: Duration, _span: &Span| {
                debug!("Sending {} bytes", chunk.len())
            })
            .on_eos(
                |_trailers: Option<&HeaderMap>, stream_duration: Duration, _span: &Span| {
                    debug!("Stream closed after {:?}", stream_duration)
                },
            )
            .on_failure(
                |error: ServerErrorsFailureClass, latency: Duration, _span: &Span| {
                    error!(
                        "Something went wrong {} in {:?}",
                        error.to_string(),
                        latency
                    )
                },
            ),
    )
}
