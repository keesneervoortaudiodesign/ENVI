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
    /// 409 — the `recondition` tensor-hash mismatch (SC4, D-07). The `body` is
    /// the **frozen top-level 409 shape** `{ error: "tensor_hash_mismatch",
    /// expected, got, hint }` and is served verbatim (NOT wrapped in the
    /// `{ error, detail }` envelope) so the contract body is exactly as designed.
    Conflict {
        /// The pre-built, frozen 409 JSON body served verbatim.
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
        // The 409 tensor-hash-mismatch body is the frozen SC4 contract shape and
        // is served verbatim, NOT wrapped in the { error, detail } envelope.
        if let ApiError::Conflict { body } = self {
            return (StatusCode::CONFLICT, Json(body)).into_response();
        }
        let (status, code, detail) = match self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found", Value::Null),
            ApiError::BadRequest { detail } => (
                StatusCode::BAD_REQUEST,
                "bad_request",
                Value::String(detail),
            ),
            ApiError::Internal { detail } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                Value::String(detail),
            ),
            // Handled above; unreachable here.
            ApiError::Conflict { .. } => unreachable!("Conflict handled above"),
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
            // These variants embed absolute filesystem paths (and, for
            // PathEscape, the canonicalized store-root layout) in their `Display`.
            // Log the full detail server-side, but return a GENERIC client-facing
            // message so 500 bodies never leak server paths or internal error text
            // (MED-1 information-disclosure fix).
            StoreError::Io { .. } | StoreError::Json { .. } | StoreError::PathEscape { .. } => {
                tracing::error!(error = %e, "internal storage error");
                ApiError::Internal {
                    detail: "internal error".to_string(),
                }
            }
        }
    }
}
