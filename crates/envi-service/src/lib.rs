//! # envi-service
//!
//! The ENVI self-hosted HTTP service: **one deployable axum binary** that serves
//! the `/api/v1` JSON/GeoJSON API AND the `web/dist` frontend bundle (SVC-03,
//! SVC-04). This is the deployable skeleton every later phase binds to.
//!
//! # Boundary statement (thin HTTP layer)
//!
//! Handlers are **thin delegates**: HTTP parsing, validation, and status mapping
//! only. Storage lives in [`envi_store`], reprojection in [`envi_geo`], and
//! acoustic math NEVER here — the frequency axis and every spectrum originate
//! server-side from `envi_engine`'s frozen constants (SVC-07). No client ever
//! receives an acoustic computation to run.
//!
//! # Startup contract (D-08, SC2 ADJUSTED)
//!
//! The binary refuses to start unless [`selfcheck::crs_self_check`] passes a
//! pure-Rust CRS landmark round-trip (<= 1 m). This REPLACES the roadmap's literal
//! "GDAL/PROJ self-check" — GDAL provisioning is deferred to Phase 8 with the C
//! dependency (D-02); Phase 6 proves the pure-Rust CRS seam at every startup.
//!
//! # Layout
//!
//! - [`selfcheck`] — the D-08 refuse-to-start CRS round-trip.
//! - [`state`] — [`state::AppState`] (the `Arc`-shared store handle).
//! - [`error`] — [`error::ApiError`] -> `IntoResponse` (status + structured JSON).
//! - [`api`] — the `/api/v1` router (axum 0.8 brace-syntax paths) + handlers.
//!
//! # House rules
//! - `f64` throughout; typed errors, never panics on data.
//! - axum 0.8 path params use **brace** syntax `/{id}` — the 0.7 colon syntax
//!   panics at router construction (Pitfall 2).
//!
#![deny(unsafe_code)]

pub mod api;
pub mod error;
pub mod selfcheck;
pub mod state;
