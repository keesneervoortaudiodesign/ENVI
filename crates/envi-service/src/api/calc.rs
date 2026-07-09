//! Calculation endpoints: the SC4 recondition/recompute structural split with the
//! enforced 409 content-hash gate (06-RESEARCH Pattern 5, D-07).
//!
//! # The split (SC4, D-07)
//!
//! Tensor identity is a **content hash** over geometry + met + receivers only —
//! conditioning is a readout parameter and is structurally excluded (it never
//! reaches [`envi_store::hash::tensor_hash`], whose signature accepts no
//! conditioning type). Two structurally separate endpoints bind to that identity:
//!
//! - `recondition` (conditioning -> MAC readout): its request carries the
//!   `tensor_hash` the client believes it is reconditioning. A **mismatch** —
//!   the scene/met/receivers changed since the tensor was minted — is ACTUALLY
//!   rejected with **HTTP 409** carrying `expected`/`got`/`hint` (never silently
//!   served stale). A match returns dense `[105]` band-index spectra per receiver,
//!   flagged `stub: true`.
//! - `recompute` (scene/terrain/ground/met -> propagation): the separate path that
//!   re-mints identity from the CURRENT scene and launches a fresh job (202).
//!
//! # Scope guard (D-07)
//!
//! This freezes only the split + hash identity + 409. The full dirty-diff recalc
//! **router (Tiers 0-3)** — deciding which propagation work a given edit actually
//! invalidates — is Phase 10/11. No tier/dirty-diff routing logic lives here.
//!
//! # Honest stubs
//!
//! Every canned spectrum is a deterministic, obviously-synthetic all-zero `[105]`
//! array and every response/manifest carries `stub: true`; nothing here can read
//! as a validated acoustic result (CONTEXT honest-stubs rule). The real MAC
//! readout (`compose_gain`) is Phase 11.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use geojson::FeatureCollection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use envi_geo::{LonLat, ProjectCrs};
use envi_store::dto::{BandSpectrumDto, ConditioningDto, ReceiverDto};
use envi_store::hash::tensor_hash;
use envi_store::manifest::{CalcManifest, chunk_receivers, write_manifest};

use envi_engine::freq::N_BANDS;

use crate::error::ApiError;
use crate::jobs::{StubJobSpec, submit_stub_job};
use crate::state::{AppState, CalcRecord};

/// A modest stub job spec for calc submit/recompute — long enough to observe the
/// SC5 machine live, short enough for interactive/test latency.
const CALC_JOB_SPEC: StubJobSpec = StubJobSpec {
    steps: 12,
    step_ms: 20,
    fail_at: None,
};

// ---------------------------------------------------------------------------
// Request / response DTOs (frozen wire shapes)
// ---------------------------------------------------------------------------

/// `POST /calculations/{cid}/recondition` body. Strict (`deny_unknown_fields`).
///
/// `conditioning` is a per-source **readout** map — it is deserialized and
/// validated (filter arrays must be dense `[105]` when present) but influences
/// NOTHING in Phase 6 (D-07: it never enters tensor identity; the MAC readout is
/// Phase 11).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconditionRequest {
    /// The tensor identity the client believes it is reconditioning.
    pub tensor_hash: String,
    /// Per-source conditioning (gain/delay/filter/mute), keyed by source uuid.
    #[serde(default)]
    pub conditioning: HashMap<Uuid, ConditioningDto>,
}

/// The reason a `recompute` was requested. Minimal + extensible: new reasons can
/// be added without breaking old clients (06-RESEARCH Open Q1). `snake_case` wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecomputeReason {
    /// Scene geometry changed.
    Geometry,
    /// Meteorology changed.
    Met,
    /// The receiver set changed.
    Receivers,
}

/// `POST /calculations/{cid}/recompute` body — minimal frozen shape. Extensible
/// via future `#[serde(default)]` fields (never breaking).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecomputeRequest {
    /// Why the recompute was requested (drives the Phase-10/11 tier router later).
    pub reason: RecomputeReason,
}

/// `POST /projects/{id}/calculations` -> 202 body.
#[derive(Debug, Clone, Serialize)]
pub struct SubmitResponse {
    /// The new calculation id (also the `calc/<id>/` folder name).
    pub calc_id: Uuid,
    /// The stub job driving the SC5 state machine for this calculation.
    pub job_id: Uuid,
    /// The minted tensor identity (geometry + met + receivers; D-07).
    pub tensor_hash: String,
}

/// `POST /calculations/{cid}/recondition` -> 200 body (match path).
#[derive(Debug, Clone, Serialize)]
pub struct ReconditionResponse {
    /// One canned dense `[105]` band-index spectrum per scene receiver uuid.
    pub spectra: HashMap<Uuid, BandSpectrumDto>,
    /// The tensor identity these spectra read out of (echoed).
    pub tensor_hash: String,
    /// Honest-stub provenance — these are synthetic, not validated acoustics.
    pub stub: bool,
}

/// `POST /calculations/{cid}/recompute` -> 202 body.
#[derive(Debug, Clone, Serialize)]
pub struct RecomputeResponse {
    /// The stub job driving the re-computation.
    pub job_id: Uuid,
    /// The freshly minted tensor identity from the CURRENT scene/met/receivers.
    pub tensor_hash: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /projects/{id}/calculations` -> 202 `{ calc_id, job_id, tensor_hash }`.
///
/// Loads the project's scene + met + receivers, mints the content-hash tensor
/// identity (D-07), writes an honest-stub [`CalcManifest`], registers the
/// in-memory [`CalcRecord`] (the stub tensor the 409 gate checks against), and
/// launches the SC5 stub job.
pub async fn submit(
    State(app): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
) -> Result<(StatusCode, Json<SubmitResponse>), ApiError> {
    let identity = mint_identity(&app, project_id)?;

    let calc_id = Uuid::new_v4();
    let manifest = CalcManifest {
        calc_id,
        dims: identity.dims,
        chunk_receivers: chunk_receivers(identity.dims[0], identity.dims[1]),
        tensor_hash: identity.tensor_hash.clone(),
        stub: true,
        created_at_unix: now_unix(),
    };
    write_manifest(&app.store.project_dir(project_id), &manifest)?;

    app.calcs.write().await.insert(
        calc_id,
        CalcRecord {
            project_id,
            tensor_hash: identity.tensor_hash.clone(),
            dims: identity.dims,
        },
    );

    let job_id = submit_stub_job(&app, CALC_JOB_SPEC).await;

    Ok((
        StatusCode::ACCEPTED,
        Json(SubmitResponse {
            calc_id,
            job_id: job_id.0,
            tensor_hash: identity.tensor_hash,
        }),
    ))
}

/// `POST /calculations/{cid}/recondition` -> 200 canned spectra | 409 mismatch.
///
/// The 409 body is the frozen SC4 shape `{ error: "tensor_hash_mismatch",
/// expected, got, hint }` (D-07; designed-ahead SVC-06, realized fully Phase 11).
/// On a hash match, returns one deterministic all-zero `[105]` spectrum per scene
/// receiver, flagged `stub: true`. The stub tensor is NEVER mutated by a
/// recondition — a mismatched request leaves it intact for a later matched call.
pub async fn recondition(
    State(app): State<Arc<AppState>>,
    Path(calc_id): Path<Uuid>,
    Json(req): Json<ReconditionRequest>,
) -> Result<Json<ReconditionResponse>, ApiError> {
    // Look up the stub tensor identity (404 for an unknown calc).
    let record = {
        let calcs = app.calcs.read().await;
        calcs.get(&calc_id).cloned().ok_or(ApiError::NotFound)?
    };

    // Validate conditioning shape (readout param): filter arrays must be dense
    // [105] when present. This influences nothing in Phase 6 (D-07).
    for (source_id, cond) in &req.conditioning {
        if let Some(filter) = &cond.filter_band_db
            && filter.len() != N_BANDS
        {
            return Err(ApiError::BadRequest {
                detail: format!(
                    "conditioning[{source_id}].filter_band_db has {} values, expected {N_BANDS} (dense [105] by band index)",
                    filter.len()
                ),
            });
        }
    }

    // The enforced 409 gate (SC4): a mismatched hash is ACTUALLY rejected.
    if req.tensor_hash != record.tensor_hash {
        return Err(ApiError::Conflict {
            body: json!({
                "error": "tensor_hash_mismatch",
                "expected": record.tensor_hash,
                "got": req.tensor_hash,
                "hint": format!(
                    "scene/met/receivers changed — POST /api/v1/calculations/{calc_id}/recompute"
                ),
            }),
        });
    }

    // Match: one canned dense [105] band-index spectrum per scene receiver uuid.
    let scene = app.store.load_scene(record.project_id)?;
    let receiver_ids = scene_receiver_ids(&scene)?;
    let mut spectra = HashMap::with_capacity(receiver_ids.len());
    for id in receiver_ids {
        spectra.insert(
            id,
            BandSpectrumDto {
                band_db: vec![0.0; N_BANDS],
            },
        );
    }

    Ok(Json(ReconditionResponse {
        spectra,
        tensor_hash: record.tensor_hash,
        stub: true,
    }))
}

/// `POST /calculations/{cid}/recompute` -> 202 `{ job_id, tensor_hash }`.
///
/// Re-mints tensor identity from the project's CURRENT scene/met/receivers,
/// updates the [`CalcRecord`] + rewrites the manifest, and launches a fresh stub
/// job. This endpoint freezes ONLY the split (D-07 scope guard) — the Tier-0..3
/// dirty-diff router that decides *what* to recompute is Phase 10/11.
pub async fn recompute(
    State(app): State<Arc<AppState>>,
    Path(calc_id): Path<Uuid>,
    Json(_req): Json<RecomputeRequest>,
) -> Result<(StatusCode, Json<RecomputeResponse>), ApiError> {
    // The calc must exist (404 otherwise); capture its project.
    let project_id = {
        let calcs = app.calcs.read().await;
        calcs.get(&calc_id).ok_or(ApiError::NotFound)?.project_id
    };

    let identity = mint_identity(&app, project_id)?;

    let manifest = CalcManifest {
        calc_id,
        dims: identity.dims,
        chunk_receivers: chunk_receivers(identity.dims[0], identity.dims[1]),
        tensor_hash: identity.tensor_hash.clone(),
        stub: true,
        created_at_unix: now_unix(),
    };
    write_manifest(&app.store.project_dir(project_id), &manifest)?;

    app.calcs.write().await.insert(
        calc_id,
        CalcRecord {
            project_id,
            tensor_hash: identity.tensor_hash.clone(),
            dims: identity.dims,
        },
    );

    let job_id = submit_stub_job(&app, CALC_JOB_SPEC).await;

    Ok((
        StatusCode::ACCEPTED,
        Json(RecomputeResponse {
            job_id: job_id.0,
            tensor_hash: identity.tensor_hash,
        }),
    ))
}

// ---------------------------------------------------------------------------
// Identity helpers
// ---------------------------------------------------------------------------

/// The minted tensor identity for a project's current state.
struct Identity {
    tensor_hash: String,
    /// `[S, R, 105]` — `S >= 1` (at least one sub-source axis), `R` the receiver
    /// count, `F` always [`N_BANDS`].
    dims: [usize; 3],
}

/// Load a project's scene + met + receivers and mint its content-hash tensor
/// identity (geometry + met + receivers ONLY — conditioning excluded, D-07).
fn mint_identity(app: &AppState, project_id: Uuid) -> Result<Identity, ApiError> {
    let meta = app.store.load_meta(project_id)?;
    let scene = app.store.load_scene(project_id)?;
    let crs = meta.crs.to_project_crs()?;
    let met = meta.settings.met.clone();

    let receivers = scene_receivers(&scene, &crs)?;
    let source_count = scene_source_count(&scene);

    let tensor_hash = tensor_hash(&scene, &met, &receivers);
    let dims = [source_count.max(1), receivers.len(), N_BANDS];

    Ok(Identity { tensor_hash, dims })
}

/// Extract the receiver-set (reprojected to SceneXY, id-tagged) from the scene —
/// the receiver axis of tensor identity (D-07).
fn scene_receivers(
    scene: &FeatureCollection,
    crs: &ProjectCrs,
) -> Result<Vec<ReceiverDto>, ApiError> {
    let mut out = Vec::new();
    for feature in &scene.features {
        let Some(props) = feature.properties.as_ref() else {
            continue;
        };
        if props.get("kind").and_then(|v| v.as_str()) != Some("receiver") {
            continue;
        }
        let id = props
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| ApiError::BadRequest {
                detail: "receiver feature is missing a valid uuid id".to_string(),
            })?;
        let Some(geometry) = feature.geometry.as_ref() else {
            continue;
        };
        let geojson::GeometryValue::Point { coordinates } = &geometry.value else {
            return Err(ApiError::BadRequest {
                detail: "receiver feature must be a Point".to_string(),
            });
        };
        if coordinates.len() < 2 {
            return Err(ApiError::BadRequest {
                detail: "receiver position needs >= 2 components".to_string(),
            });
        }
        let lonlat = LonLat {
            lon_deg: coordinates[0],
            lat_deg: coordinates[1],
        };
        let xy = crs.to_utm(lonlat).map_err(|e| ApiError::BadRequest {
            detail: e.to_string(),
        })?;
        let z = props
            .get("height_m")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        out.push(ReceiverDto {
            id,
            position: [xy.x_m, xy.y_m, z],
        });
    }
    Ok(out)
}

/// Count `source`-kind features (the S axis; at least 1 after `max(1, ..)`).
fn scene_source_count(scene: &FeatureCollection) -> usize {
    scene
        .features
        .iter()
        .filter(|f| {
            f.properties
                .as_ref()
                .and_then(|p| p.get("kind"))
                .and_then(|v| v.as_str())
                == Some("source")
        })
        .count()
}

/// Collect the uuids of every `receiver`-kind feature (spectra readout keys).
fn scene_receiver_ids(scene: &FeatureCollection) -> Result<Vec<Uuid>, ApiError> {
    let mut ids = Vec::new();
    for feature in &scene.features {
        let Some(props) = feature.properties.as_ref() else {
            continue;
        };
        if props.get("kind").and_then(|v| v.as_str()) != Some("receiver") {
            continue;
        }
        let id = props
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| ApiError::BadRequest {
                detail: "receiver feature is missing a valid uuid id".to_string(),
            })?;
        ids.push(id);
    }
    Ok(ids)
}

/// Current unix epoch seconds (matches the store's timestamp convention).
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
