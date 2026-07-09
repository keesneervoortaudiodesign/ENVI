//! The HTTP error boundary: [`ApiError`] -> `IntoResponse` (06-PATTERNS
//! Archetype C; the `ApiError` type has no codebase analog, built per
//! 06-RESEARCH).
//!
//! Every fallible handler returns `Result<_, ApiError>`. Store-layer failures map
//! to HTTP status via [`From<StoreError>`], and the response body is always the
//! structured JSON `{ "error": "<code>", "detail": <value> }` — never a bare
//! string or an HTML page (the API namespace stays JSON, T-06-03).

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use envi_store::StoreError;

/// A typed API error that renders as a structured JSON body with the right HTTP
/// status. Reuses the got/expected field style (Archetype A) so future 409
/// bodies can carry `expected`/`got` hashes.
#[derive(Debug)]
pub enum ApiError {
    /// 404 — no such project/scene/resource.
    NotFound,
    /// 400 — the request was malformed or violated a validation rule.
    BadRequest {
        /// Human-readable reason (safe to surface to the client).
        detail: String,
    },
    /// 409 — reserved for plan 06-04's `recondition` tensor-hash mismatch. The
    /// body carries the structured mismatch detail (`expected`/`got`/`hint`).
    #[allow(dead_code, reason = "constructed by the recondition 409 path in plan 06-04")]
    Conflict {
        /// The pre-built JSON conflict body.
        body: Value,
    },
    /// 500 — an unexpected internal failure (filesystem, serialization).
    Internal {
        /// Human-readable reason (logged; also surfaced in the body).
        detail: String,
    },
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, detail) = match self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found", Value::Null),
            ApiError::BadRequest { detail } => {
                (StatusCode::BAD_REQUEST, "bad_request", Value::String(detail))
            }
            ApiError::Conflict { body } => (StatusCode::CONFLICT, "conflict", body),
            ApiError::Internal { detail } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                Value::String(detail),
            ),
        };
        (status, Json(json!({ "error": code, "detail": detail }))).into_response()
    }
}

impl From<StoreError> for ApiError {
    /// Map a persistence-layer error to the appropriate HTTP status.
    ///
    /// - Missing project/calc -> 404.
    /// - Validation failures (bad kind, missing property, band count, non-finite,
    ///   GeoJSON schema, reprojection, engine rejection) -> 400 (client fault).
    /// - Filesystem / serialization / path-escape faults -> 500 (server fault).
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound { .. } | StoreError::CalcNotFound { .. } => ApiError::NotFound,
            StoreError::UnknownKind { .. }
            | StoreError::MissingProperty { .. }
            | StoreError::BadBandCount { .. }
            | StoreError::NonFinite { .. }
            | StoreError::GeoJson { .. }
            | StoreError::Geo(_)
            | StoreError::Engine { .. } => ApiError::BadRequest {
                detail: e.to_string(),
            },
            StoreError::Io { .. } | StoreError::Json { .. } | StoreError::PathEscape { .. } => {
                ApiError::Internal {
                    detail: e.to_string(),
                }
            }
        }
    }
}
