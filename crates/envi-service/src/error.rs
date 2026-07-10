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

use envi_dgm::DgmError;
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
        let (status, body) = match self {
            // The 409 tensor-hash-mismatch body is the frozen SC4 contract shape
            // and is served VERBATIM at the top level, NOT wrapped in the
            // { error, detail } envelope.
            ApiError::Conflict { body } => (StatusCode::CONFLICT, body),
            ApiError::NotFound => (
                StatusCode::NOT_FOUND,
                json!({ "error": "not_found", "detail": Value::Null }),
            ),
            ApiError::BadRequest { detail } => (
                StatusCode::BAD_REQUEST,
                json!({ "error": "bad_request", "detail": detail }),
            ),
            // MED-1: the generic "internal error" detail is already set at the
            // From<StoreError> boundary (which logs the full error via
            // tracing::error!), so a filesystem path never reaches this body.
            ApiError::Internal { detail } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "internal", "detail": detail }),
            ),
        };
        (status, Json(body)).into_response()
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

impl From<DgmError> for ApiError {
    /// Map a digital-ground-model failure to HTTP status (D-08, Pitfall 3).
    ///
    /// EVERY `DgmError` variant is a **client fault** — the untrusted elevation
    /// points / breaklines were degenerate, non-finite, oversized, or
    /// self-crossing — so all map to `400`, never `500` and never a panic. The
    /// `Display` text carries only offending coordinates / counts (no filesystem
    /// paths), so it is safe to surface verbatim in the `BadRequest { detail }`
    /// body. `envi_dgm::build_tin` already pre-checks crossing breaklines
    /// (`can_add_constraint`) so `spade` never panics the request thread.
    fn from(e: DgmError) -> Self {
        match e {
            DgmError::TooFewPoints { .. }
            | DgmError::IntersectingConstraint { .. }
            | DgmError::NonFinite { .. }
            | DgmError::TooLarge { .. }
            | DgmError::Triangulation { .. } => ApiError::BadRequest {
                detail: e.to_string(),
            },
        }
    }
}
