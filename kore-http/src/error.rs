use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

// Error
pub enum Error {
    Kore(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::Kore(error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response()
            }
        }
    }
}
