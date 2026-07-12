//! No-drift guard for the generated TypeScript wire contract (D-10).
//!
//! # Module I/O
//! - **Input:** every `ts_rs::TS`-deriving wire DTO across `envi-store`
//!   (16 DTOs + [`Resolution`]), `envi-service` (18 request/response/enum
//!   types incl. [`JobStatus`]), and the `envi-gis-wasm` WASM ingestion boundary
//!   (the `envi_gis_wasm::dto` request/result DTOs, DATA-01..03), plus the
//!   committed `web/src/generated/wire.ts`. The WASM boundary rides the SAME
//!   generate-and-commit mechanism as the HTTP wire — one committed artifact,
//!   one no-drift test, one source of truth (Phase-7 D-10).
//! - **Output:** a `#[test]` verdict. Regenerates the whole wire contract into a
//!   `TempDir` and asserts it equals the committed artifact byte-for-byte
//!   (newline-normalized). A renamed / added / removed Rust field — or any serde
//!   attribute change that alters the TS shape — makes the regenerated string
//!   diverge, so `cargo test` fails in Rust rather than the browser.
//! - **Valid input range:** the committed `wire.ts` must exist; regenerate it
//!   with `cargo test -p envi-service --test wire_no_drift -- --ignored \
//!   regenerate_committed_wire_ts` after intentionally changing a DTO.
//!
//! # Why this mirrors the oracle-fixture pattern (07-PATTERNS call-out a)
//!
//! This is the `tools/nord2000_oracle/` "generate-at-dev-time, commit-the-
//! artifact, test-asserts-no-drift" contract (`crates/harness/tests/
//! oracle_ground.rs`) inverted: instead of loading a committed fixture and
//! comparing engine output to it, we regenerate the artifact and compare it to
//! the committed copy. Same guarantee — the committed file is the source of
//! truth at build time, the generator is not needed to *pass*, and drift fails
//! the build.
//!
//! # Single-file, deterministic generation (not the `#[ts(export)]` auto-test)
//!
//! The ~32 wire types span two crates. ts-rs's `#[ts(export)]` auto-tests run in
//! each crate's own test binary and would race two separate processes writing
//! one shared file (and resolve their output path relative to each crate's
//! `bindings/` dir). Instead every type carries only `#[ts(export_to =
//! "wire.ts")]`, and this single test binary — which sees BOTH crates' types —
//! drives one explicit [`TS::export_all`] pass into a chosen directory. ts-rs
//! merges every declaration into one `wire.ts`, inserted alphabetically, so the
//! output is byte-stable regardless of call order.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use ts_rs::{Config, TS};

use envi_service::api::calc::{
    RecomputeReason, RecomputeRequest, RecomputeResponse, ReconditionRequest, ReconditionResponse,
    SubmitResponse,
};
use envi_service::api::dgm::{DgmReq, DgmResp};
use envi_service::api::meta::{
    FreqAxisDto, InterpolateReq, InterpolateResp, SplToLwReq, SplToLwResp,
};
use envi_service::api::projects::{CreateProjectRequest, OriginDto, UpdateProjectRequest};
use envi_service::jobs::{JobId, JobStatus};
use envi_store::dto::{
    AuthoredSpectrumDto, BandSpectrumDto, BarrierDto, BuildingDto, ConditioningDto, CrsDto,
    ForestParamsDto, GroundSegmentDto, IsolationSpectrumDto, MetDto, ProjectMetaDto, ReceiverDto,
    SettingsDto, SourceDto, SubSourceDto, TerrainProfileDto,
};
use envi_store::interpolate::Resolution;

// The envi-gis-wasm WASM ingestion-boundary DTOs (DATA-01..03) — generated into
// the SAME committed wire.ts as the HTTP wire (D-10).
use envi_gis_wasm::dto::{
    BaseElevationReq, BaseElevationResult, BboxDto, BuildingsResult, ClassOccurrenceDto, CorsDto,
    CutProfileReq, CutProfileResult, DecodeWindowReq, DecodeWindowResult, DrawnZoneDto,
    Era5DeriveReq, Era5DeriveResult, Era5HourDto, GeoTransformDto, GroundSegmentationDto,
    ImportPlanReq, ImportPlanResult, ImportedZoneDto, InjectScreensReq, LandcoverResult,
    MapLandcoverReq, MergeReq, MergeResult, ParseBuildingsReq, PixelWindowDto, PlanTilesReq,
    PlanTilesResult, ProfileSegmentDto, ProvenanceReqDto, ReceiverGridReq, ReceiverGridResult,
    ReprojectRingReq, ReprojectRingResult, ScreenObjectDto, SegmentGroundReq, SkipReportDto,
    SoundSpeedProfileDto, SourceDescriptorDto, SourceKindDto, TerrainFeaturesReq,
    TerrainFeaturesResult, TerrainSourceCrsDto, TileRefDto, VerticalDatumDto, WeatherComponentsDto,
    WeatherDeriveReq, WeatherDeriveResult, WindowForBboxReq, WindowForBboxResult,
};

// The envi-compute-wasm browser compute-boundary DTOs (SVC-02 / GRID-02, plan
// 10-03) — cost estimate + hierarchical tier partition + the TierComplete D-07
// event payload. Generated into the SAME committed wire.ts as the HTTP wire and
// the GIS boundary (D-10). The reused JobStatus union is the envi-service one
// above (this crate defines no duplicate).
use envi_compute_wasm::dto::{
    AtmosphereDto, ChunkSpanDto, CoherenceInputsDto, CostEstimateResult, DirectionalDto,
    DirectivityBalloonDto, EstimateCostReq, ExportCrsDto, ExportFormat, ExportGridDto, ExportReq,
    GuardrailLevelDto, PlanTiersReq, PrepareSolveReq, RangeProgressDto, ReadoutResult,
    ReceiverPlacementDto, ReceiverReadoutDto, ReconditionReq, ReconditionResult, RotationDto,
    SolveChunkRangeReq, SubSourcePlacementDto, TierComplete, TierDto, TierKindDto, TierPlanResult,
    TierReceiverDto, TraceIsophonesReq,
};

/// Provenance banner prepended to the committed `wire.ts` (mirrors the oracle
/// fixtures' `# generated by … — DO NOT EDIT` header). Prepended by the SAME
/// generator on every run, so it is part of the byte-equality contract.
const BANNER: &str = "\
// GENERATED from the envi-store + envi-service serde DTOs via ts-rs (D-10). DO NOT EDIT.
// Regenerate: cargo test -p envi-service --test wire_no_drift -- --ignored regenerate_committed_wire_ts
// A renamed/added/removed Rust field fails the wire_no_drift test, not the browser.
";

/// Export every wire type (and, transitively, its dependencies) into `cfg`'s
/// output directory. Because every type's `#[ts(export_to)]` names the same
/// `wire.ts`, ts-rs merges them into one file. Listing the types explicitly (not
/// just roots) keeps the set auditable — a new wire DTO must be added here.
fn export_all_wire_types(cfg: &Config) {
    // envi-store DTOs + the Resolution enum.
    BandSpectrumDto::export_all(cfg).unwrap();
    SubSourceDto::export_all(cfg).unwrap();
    SourceDto::export_all(cfg).unwrap();
    ReceiverDto::export_all(cfg).unwrap();
    BarrierDto::export_all(cfg).unwrap();
    BuildingDto::export_all(cfg).unwrap();
    GroundSegmentDto::export_all(cfg).unwrap();
    TerrainProfileDto::export_all(cfg).unwrap();
    AuthoredSpectrumDto::export_all(cfg).unwrap();
    IsolationSpectrumDto::export_all(cfg).unwrap();
    ForestParamsDto::export_all(cfg).unwrap();
    CrsDto::export_all(cfg).unwrap();
    MetDto::export_all(cfg).unwrap();
    SettingsDto::export_all(cfg).unwrap();
    ProjectMetaDto::export_all(cfg).unwrap();
    ConditioningDto::export_all(cfg).unwrap();
    Resolution::export_all(cfg).unwrap();

    // envi-service request / response / enum wire types.
    JobStatus::export_all(cfg).unwrap();
    JobId::export_all(cfg).unwrap();
    FreqAxisDto::export_all(cfg).unwrap();
    InterpolateReq::export_all(cfg).unwrap();
    InterpolateResp::export_all(cfg).unwrap();
    SplToLwReq::export_all(cfg).unwrap();
    SplToLwResp::export_all(cfg).unwrap();
    DgmReq::export_all(cfg).unwrap();
    DgmResp::export_all(cfg).unwrap();
    OriginDto::export_all(cfg).unwrap();
    CreateProjectRequest::export_all(cfg).unwrap();
    UpdateProjectRequest::export_all(cfg).unwrap();
    SubmitResponse::export_all(cfg).unwrap();
    ReconditionRequest::export_all(cfg).unwrap();
    ReconditionResponse::export_all(cfg).unwrap();
    RecomputeRequest::export_all(cfg).unwrap();
    RecomputeResponse::export_all(cfg).unwrap();
    RecomputeReason::export_all(cfg).unwrap();

    // envi-gis-wasm WASM ingestion-boundary DTOs (DATA-01..03). A new boundary
    // DTO must be added here — same auditable-list discipline as the HTTP wire.
    // Shared value objects + enums.
    PixelWindowDto::export_all(cfg).unwrap();
    BboxDto::export_all(cfg).unwrap();
    TerrainSourceCrsDto::export_all(cfg).unwrap();
    VerticalDatumDto::export_all(cfg).unwrap();
    ProvenanceReqDto::export_all(cfg).unwrap();
    // Request DTOs.
    ImportPlanReq::export_all(cfg).unwrap();
    PlanTilesReq::export_all(cfg).unwrap();
    WindowForBboxReq::export_all(cfg).unwrap();
    ReprojectRingReq::export_all(cfg).unwrap();
    DecodeWindowReq::export_all(cfg).unwrap();
    TerrainFeaturesReq::export_all(cfg).unwrap();
    BaseElevationReq::export_all(cfg).unwrap();
    MapLandcoverReq::export_all(cfg).unwrap();
    ParseBuildingsReq::export_all(cfg).unwrap();
    MergeReq::export_all(cfg).unwrap();
    // Result DTOs.
    CorsDto::export_all(cfg).unwrap();
    SourceKindDto::export_all(cfg).unwrap();
    SourceDescriptorDto::export_all(cfg).unwrap();
    ImportPlanResult::export_all(cfg).unwrap();
    GeoTransformDto::export_all(cfg).unwrap();
    DecodeWindowResult::export_all(cfg).unwrap();
    TerrainFeaturesResult::export_all(cfg).unwrap();
    BaseElevationResult::export_all(cfg).unwrap();
    LandcoverResult::export_all(cfg).unwrap();
    SkipReportDto::export_all(cfg).unwrap();
    BuildingsResult::export_all(cfg).unwrap();
    MergeResult::export_all(cfg).unwrap();
    TileRefDto::export_all(cfg).unwrap();
    PlanTilesResult::export_all(cfg).unwrap();
    WindowForBboxResult::export_all(cfg).unwrap();
    ReprojectRingResult::export_all(cfg).unwrap();

    // envi-gis-wasm Phase-9 geometry + weather boundary DTOs (GEOX/GRID/METX).
    // Shared value objects.
    ProfileSegmentDto::export_all(cfg).unwrap();
    GroundSegmentationDto::export_all(cfg).unwrap();
    DrawnZoneDto::export_all(cfg).unwrap();
    ImportedZoneDto::export_all(cfg).unwrap();
    ScreenObjectDto::export_all(cfg).unwrap();
    WeatherComponentsDto::export_all(cfg).unwrap();
    SoundSpeedProfileDto::export_all(cfg).unwrap();
    Era5HourDto::export_all(cfg).unwrap();
    ClassOccurrenceDto::export_all(cfg).unwrap();
    // Request DTOs.
    CutProfileReq::export_all(cfg).unwrap();
    SegmentGroundReq::export_all(cfg).unwrap();
    InjectScreensReq::export_all(cfg).unwrap();
    ReceiverGridReq::export_all(cfg).unwrap();
    WeatherDeriveReq::export_all(cfg).unwrap();
    Era5DeriveReq::export_all(cfg).unwrap();
    // Result DTOs.
    CutProfileResult::export_all(cfg).unwrap();
    ReceiverGridResult::export_all(cfg).unwrap();
    WeatherDeriveResult::export_all(cfg).unwrap();
    Era5DeriveResult::export_all(cfg).unwrap();

    // envi-compute-wasm compute-boundary DTOs (SVC-02 / GRID-02, plan 10-03).
    // A new compute-boundary DTO must be added here — same auditable-list
    // discipline. JobStatus is NOT re-registered (reused from envi-service above).
    // Request DTOs.
    EstimateCostReq::export_all(cfg).unwrap();
    PlanTiersReq::export_all(cfg).unwrap();
    SolveChunkRangeReq::export_all(cfg).unwrap();
    // prepare_solve request + its engine-type marshalling DTOs (10-06). The scene
    // DTOs it references (Terrain/Ground/Isolation/Forest/SoundSpeedProfile) are
    // registered above via envi-store / envi-gis-wasm (one wire type each).
    PrepareSolveReq::export_all(cfg).unwrap();
    AtmosphereDto::export_all(cfg).unwrap();
    CoherenceInputsDto::export_all(cfg).unwrap();
    ReceiverPlacementDto::export_all(cfg).unwrap();
    SubSourcePlacementDto::export_all(cfg).unwrap();
    DirectionalDto::export_all(cfg).unwrap();
    DirectivityBalloonDto::export_all(cfg).unwrap();
    RotationDto::export_all(cfg).unwrap();
    RangeProgressDto::export_all(cfg).unwrap();
    // Result / value DTOs.
    GuardrailLevelDto::export_all(cfg).unwrap();
    CostEstimateResult::export_all(cfg).unwrap();
    TierKindDto::export_all(cfg).unwrap();
    TierReceiverDto::export_all(cfg).unwrap();
    TierDto::export_all(cfg).unwrap();
    TierPlanResult::export_all(cfg).unwrap();
    // Tier-complete event payload (D-07).
    ChunkSpanDto::export_all(cfg).unwrap();
    TierComplete::export_all(cfg).unwrap();
    // Recondition MAC request/result (SVC-06 / D-01, 11-03). ConditioningDto (which
    // ReconditionReq reuses) is registered above via envi-store — one wire type.
    ReconditionReq::export_all(cfg).unwrap();
    ReconditionResult::export_all(cfg).unwrap();
    // Full two-channel readout boundary (WEB-11 spectrum panel, 11-05). Reuses the
    // ReconditionReq request shape (registered above) — only the richer result +
    // per-receiver readout DTO are new wire types.
    ReceiverReadoutDto::export_all(cfg).unwrap();
    ReadoutResult::export_all(cfg).unwrap();
    // Export encoders → browser-download bytes (GRID-05 / D-20/21/22, 11-04). The
    // ReceiverReadoutDto ExportReq reuses is registered just above — one wire type.
    ExportFormat::export_all(cfg).unwrap();
    ExportCrsDto::export_all(cfg).unwrap();
    ExportGridDto::export_all(cfg).unwrap();
    ExportReq::export_all(cfg).unwrap();
    // Live isophone fill-layer tracer request (WEB-06 / GRID-04, 11-06). Reuses
    // ExportGridDto/ExportCrsDto (registered just above) — one wire type each.
    TraceIsophonesReq::export_all(cfg).unwrap();
}

/// Deterministically regenerate the full `wire.ts` contents (banner + ts-rs
/// output). Generation always targets a fresh `TempDir` so ts-rs never merges
/// into a stale file, guaranteeing byte-stable output.
fn generate_wire_ts() -> String {
    let tmp = TempDir::new().expect("temp dir");
    let cfg = Config::default().with_out_dir(tmp.path());
    export_all_wire_types(&cfg);
    let body = fs::read_to_string(tmp.path().join("wire.ts")).expect("ts-rs wrote wire.ts");
    format!("{BANNER}{body}")
}

/// The committed generated artifact, relative to this crate's manifest.
fn committed_wire_ts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/generated/wire.ts")
}

/// Newline-normalize so the byte-equality check is stable across `core.autocrlf`
/// settings (a `.gitattributes` `eol=lf` rule keeps the committed file LF, but
/// normalizing both sides makes the test robust on any contributor's checkout).
/// This normalizes ONLY line endings — every meaningful content change (a
/// renamed field, a new variant, a reordered union) still diverges and fails.
fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
}

#[test]
fn wire_ts_matches_committed_source() {
    let generated = generate_wire_ts();
    let committed = fs::read_to_string(committed_wire_ts()).unwrap_or_else(|e| {
        panic!(
            "committed web/src/generated/wire.ts must exist ({e}); regenerate with \
             `cargo test -p envi-service --test wire_no_drift -- --ignored \
             regenerate_committed_wire_ts`"
        )
    });
    assert_eq!(
        normalize(&generated),
        normalize(&committed),
        "wire.ts is out of sync with the Rust serde DTOs — a wire type changed \
         without regenerating. Run: cargo test -p envi-service --test wire_no_drift \
         -- --ignored regenerate_committed_wire_ts"
    );
}

#[test]
fn job_status_is_a_discriminated_union() {
    // Assert the committed artifact carries JobStatus as a real TS discriminated
    // union keyed on `state` (Gate 2 — ts-rs serde-compat honors #[serde(tag)]),
    // including the payload-carrying `running` variant. This is what Phase-7's
    // EventSource `switch(status.state)` handling binds to.
    let ts = normalize(&fs::read_to_string(committed_wire_ts()).expect("committed wire.ts"));
    assert!(
        ts.contains("export type JobStatus ="),
        "JobStatus must be a TS type alias (discriminated union), not an interface"
    );
    for needle in [
        "\"state\": \"queued\"",
        "\"state\": \"running\"",
        "\"state\": \"done\"",
        "\"state\": \"failed\"",
        "\"state\": \"cancelled\"",
    ] {
        assert!(
            ts.contains(needle),
            "JobStatus union must contain the variant discriminant {needle}"
        );
    }
    // The payload of the `running` and `failed` variants must survive.
    assert!(
        ts.contains("progress") && ts.contains("message"),
        "the running variant must carry `progress` and `message`"
    );
    assert!(
        ts.contains("reason"),
        "the failed variant must carry `reason`"
    );
}

/// Not a test: the committed-artifact writer. Run explicitly with `--ignored`
/// after intentionally changing a wire DTO to refresh
/// `web/src/generated/wire.ts`, then commit the result.
#[test]
#[ignore = "writer, not a check — run with --ignored to refresh the committed wire.ts"]
fn regenerate_committed_wire_ts() {
    let generated = generate_wire_ts();
    let path = committed_wire_ts();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create web/src/generated");
    }
    fs::write(&path, generated).expect("write committed wire.ts");
    eprintln!("wrote {}", path.display());
}
