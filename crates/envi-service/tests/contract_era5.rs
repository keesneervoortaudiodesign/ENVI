//! Contract tests for the flagged ERA5/CDS retrieval endpoint (D-04, METX-02,
//! T-09-04-01/02).
//!
//! Two mutually-exclusive halves, selected by the `era5` feature:
//!
//! 1. **Default build (`--features era5` OFF).** [`era5_route_absent_by_default`]
//!    drives the real [`envi_service::api::app`] router and asserts
//!    `POST /era5/import` is a structured `404` — the route is not registered when
//!    the feature is off (the endpoint-absent-by-default must-have).
//! 2. **`--features era5` build.** Unit tests pin the [`resolve_cds_upstream`]
//!    SSRF chokepoint (hardcoded host, prefix/`..` rejection before any I/O), a
//!    derivation test asserts the endpoint's occurrence statistics from committed
//!    synthetic hours (no CDS key needed), and router tests exercise the `202`
//!    happy path + the flagged-off "live retrieval disabled" `400`.
//!
//! Every case is fully OFFLINE — no socket, no CDS, no credentials.

mod common;

// ---------------------------------------------------------------------------
// Default build: the ERA5 route is ABSENT (endpoint-absent-by-default).
// ---------------------------------------------------------------------------

/// With the `era5` feature OFF, `POST /era5/import` is not registered, so it falls
/// through to the `/api/v1` JSON 404 fallback — never a `202`, never the SPA HTML.
#[cfg(not(feature = "era5"))]
#[tokio::test]
async fn era5_route_absent_by_default() {
    use axum::http::{Method, StatusCode};
    use common::{json_req, read_json, test_app};
    use tower::ServiceExt;

    let (app, _tmp) = test_app();
    let (status, json) = read_json(
        app.oneshot(json_req(
            Method::POST,
            "/api/v1/era5/import",
            &serde_json::json!({ "hours": [] }),
        ))
        .await
        .expect("router responds"),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the ERA5 route must be absent by default (feature off)"
    );
    assert_eq!(
        json["error"], "not_found",
        "structured JSON 404 envelope, not the SPA HTML shell"
    );
}

// ---------------------------------------------------------------------------
// `--features era5`: SSRF chokepoint + derivation + router happy/disabled paths.
// ---------------------------------------------------------------------------

#[cfg(feature = "era5")]
mod enabled {
    use axum::http::{Method, StatusCode};
    use tower::ServiceExt;

    use envi_service::api::era5::{Era5Hour, derive_occurrence, resolve_cds_upstream};
    use envi_service::error::ApiError;

    use crate::common::{json_req, read_json, test_app};

    /// A synthetic ERA5 hour (structural only — the `[ASSUMED]` weather-route
    /// constants stay quarantined, so this pins counts/placement, never a numeric
    /// weather pass). Physical, finite values so `obukhov` resolves.
    fn hour(ishf: f64, u10: f64, v10: f64) -> Era5Hour {
        Era5Hour {
            iews: 0.10,
            inss: 0.05,
            ishf,
            t2m_k: 288.0,
            d2m_k: 283.0,
            sp_pa: 101_325.0,
            sdfor_m: 0.0, // surely below the reliability threshold
            u10_ms: u10,
            v10_ms: v10,
        }
    }

    /// A well-formed CDS path builds the HARDCODED scheme+host URL — the caller
    /// never supplies a host, so retrieval can only ever reach the CDS host.
    #[test]
    fn resolve_cds_upstream_builds_hardcoded_https_url() {
        let url = resolve_cds_upstream("api/retrieve/v1/processes/reanalysis-era5-single-levels")
            .expect("valid CDS path resolves");
        assert_eq!(
            url,
            "https://cds.climate.copernicus.eu/api/retrieve/v1/processes/reanalysis-era5-single-levels",
            "the CDS path resolves to the hardcoded copernicus host"
        );
        assert!(url.starts_with("https://cds.climate.copernicus.eu/api/"));
    }

    /// A path outside the allowlisted prefix is a typed `BadRequest` whose detail
    /// leaks no upstream host (T-09-04-02).
    #[test]
    fn resolve_cds_upstream_rejects_prefix_escape() {
        let err = resolve_cds_upstream("etc/passwd").unwrap_err();
        match err {
            ApiError::BadRequest { detail } => assert!(
                !detail.contains("copernicus") && !detail.contains("http"),
                "prefix-escape detail must not leak the CDS host; got {detail:?}"
            ),
            other => panic!("expected BadRequest for a prefix escape, got {other:?}"),
        }
    }

    /// A `..` traversal under an otherwise valid prefix is refused before any I/O.
    #[test]
    fn resolve_cds_upstream_rejects_dot_dot_traversal() {
        let err = resolve_cds_upstream("api/../../etc/passwd").unwrap_err();
        assert!(
            matches!(err, ApiError::BadRequest { .. }),
            "a `..` traversal is a BadRequest even under a valid prefix"
        );
    }

    /// The endpoint's derivation produces the expected occurrence statistics from
    /// committed synthetic hours: two identical strong-wind daytime-unstable hours
    /// land in ONE class cell (count 2), total 2, both reliable — no CDS key.
    #[test]
    fn derivation_produces_expected_occurrence_stats() {
        // Two identical hours: strong wind, daytime (ishf < 0 ⇒ unstable).
        let hours = [hour(-120.0, 8.0, 0.0), hour(-120.0, 8.0, 0.0)];
        let occ = derive_occurrence(&hours).expect("finite hours derive");

        assert_eq!(occ.total, 2, "both hours counted");
        assert_eq!(occ.reliable, 2, "sdfor = 0 ⇒ both hours reliable");
        let sum: u32 = occ.counts.iter().flatten().copied().sum();
        assert_eq!(sum, 2, "every counted hour lands in exactly one class cell");
        let max_cell = occ.counts.iter().flatten().copied().max().unwrap_or(0);
        assert_eq!(
            max_cell, 2,
            "two identical hours fall in the SAME (wind, stability) cell"
        );
    }

    /// The `202` happy path: inline already-retrieved hours spawn a derivation job
    /// on the SC5 state machine — a `job_id` is returned, no CDS key needed.
    #[tokio::test]
    async fn import_with_inline_hours_is_202_with_job_id() {
        let (app, _tmp) = test_app();
        let body = serde_json::json!({
            "hours": [
                {"iews":0.1,"inss":0.05,"ishf":-120.0,"t2m_k":288.0,"d2m_k":283.0,
                 "sp_pa":101325.0,"sdfor_m":0.0,"u10_ms":8.0,"v10_ms":0.0}
            ]
        });
        let (status, json) = read_json(
            app.oneshot(json_req(Method::POST, "/api/v1/era5/import", &body))
                .await
                .expect("router responds"),
        )
        .await;

        assert_eq!(status, StatusCode::ACCEPTED, "inline-hours import is a 202");
        assert!(
            json["job_id"].as_str().is_some(),
            "the 202 body carries a job_id for the SC5 state machine"
        );
    }

    /// The flagged-off state: an empty-`hours` request asks for a live CDS
    /// retrieval, which is disabled without a server-side `CDS_API_KEY` — a `400`
    /// whose detail leaks no host/path, and (crucially) touches no network.
    #[tokio::test]
    async fn live_retrieval_is_disabled_without_key() {
        // The test process has no CDS_API_KEY set, so retrieval is disabled.
        assert!(
            std::env::var("CDS_API_KEY").is_err(),
            "test invariant: no CDS key in the environment"
        );
        let (app, _tmp) = test_app();
        let body = serde_json::json!({
            "hours": [],
            "dataset_path": "api/retrieve/v1/processes/reanalysis-era5-single-levels"
        });
        let (status, json) = read_json(
            app.oneshot(json_req(Method::POST, "/api/v1/era5/import", &body))
                .await
                .expect("router responds"),
        )
        .await;

        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "live retrieval is disabled without a server-side CDS key"
        );
        let detail = json["detail"].as_str().unwrap_or_default();
        assert!(
            !detail.contains("copernicus") && !detail.contains("http"),
            "the disabled detail must not leak the CDS host; got {detail:?}"
        );
    }
}
