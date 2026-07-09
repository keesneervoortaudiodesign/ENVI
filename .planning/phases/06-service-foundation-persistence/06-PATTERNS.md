# Phase 6: Service Foundation & Persistence - Pattern Map

**Mapped:** 2026-07-09
**Files analyzed:** 24 new files (3 new crates + web placeholder + oracle tool)
**Analogs found:** 17 / 24 (7 files are genuinely greenfield — axum/serde/geojson patterns come from 06-RESEARCH.md)

Phase 6 is mostly greenfield (first non-engine crates), so this map is primarily
**convention transfer** — how existing crates structure Cargo.toml, module headers,
errors, and tests — plus **exact type contracts** the new DTOs and stub-compute
endpoints must twin. Everything below was read from source this session.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/envi-geo/Cargo.toml` | config | — | `crates/envi-engine/Cargo.toml` | exact (pure-math crate manifest) |
| `crates/envi-store/Cargo.toml` | config | — | `crates/envi-harness/Cargo.toml` | exact (I/O crate manifest) |
| `crates/envi-service/Cargo.toml` | config | — | `crates/envi-harness/Cargo.toml` | role-match (adds bin + async) |
| `crates/envi-geo/src/lib.rs` | crate root + error | transform | `crates/envi-engine/src/lib.rs` | exact |
| `crates/envi-geo/src/crs.rs` | utility (domain math) | transform | `crates/envi-engine/src/propagation/ground.rs` | role-match (validated pure fn returning Result) |
| `crates/envi-geo/src/transform.rs` | utility (boundary seam) | transform | `crates/envi-engine/src/transfer.rs` (single-boundary ethos) + RESEARCH Pattern 1 | partial |
| `crates/envi-store/src/lib.rs` | crate root + error | file-I/O | `crates/envi-harness/src/lib.rs` | exact |
| `crates/envi-store/src/dto.rs` | model (serde mirror) | transform | `crates/envi-engine/src/scene.rs` (the twin) + `crates/envi-harness/src/cases/mod.rs` (serde-side shape) | exact (twins scene.rs) |
| `crates/envi-store/src/geojson.rs` | transform | file-I/O | `crates/envi-harness/src/scene_build.rs` (case-format → Scene mapping) | role-match |
| `crates/envi-store/src/project_dir.rs` | service (storage) | file-I/O | `crates/envi-harness/src/cases/mod.rs` (path + typed I/O errors) | role-match |
| `crates/envi-store/src/manifest.rs` | model | file-I/O | `crates/envi-engine/src/tensor.rs` (dims/chunk vocabulary it must mirror) | exact (contract source) |
| `crates/envi-store/src/hash.rs` | utility | transform | `crates/envi-engine/src/tensor.rs::compose_gain` (frozen-order determinism doc style) | partial |
| `crates/envi-service/src/main.rs` | binary entry | request-response | `crates/envi-harness/src/main.rs` | role-match |
| `crates/envi-service/src/error.rs` | error | request-response | `crates/envi-harness/src/cases/mod.rs::CaseLoadError` | role-match (+ RESEARCH for IntoResponse) |
| `crates/envi-service/src/selfcheck.rs` | utility | request-response | — (RESEARCH Pattern 6) | no analog |
| `crates/envi-service/src/state.rs` | provider | request-response | — (RESEARCH §Architecture) | no analog |
| `crates/envi-service/src/jobs.rs` | service (state machine) | event-driven | `crates/envi-harness/src/lib.rs::Outcome` (terminal-state enum ethos only) + RESEARCH Pattern 4 | partial |
| `crates/envi-service/src/api/*.rs` | controller | request-response | — (RESEARCH §Code Examples router/DTO) | no analog |
| `crates/envi-geo` oracle fixture test | test | — | `crates/envi-harness/tests/oracle_ground.rs` | exact |
| `tools/crs_oracle/gen_utm.py` | tool | file-I/O | `tools/nord2000_oracle/gen_ground_fixtures.py` | exact |
| `crates/envi-service/tests/*.rs` (contract tests) | test | request-response | `crates/envi-harness/tests/` layout (integration-test placement) | role-match |
| unit tests in every new module | test | — | `#[cfg(test)] mod tests` in `scene.rs`/`tensor.rs`/`freq.rs` | exact |
| `web/dist/index.html` | static asset | — | — | no analog |
| root `README.md` + `crates/README.md` (doc contract) | docs | — | root `README.md` lines 18-22 (crate table) | exact — **note: `crates/README.md` does not exist yet; the CLAUDE.md doc contract names it, so Phase 6 close-out must create it** |

## Pattern Assignments

### 1. Crate scaffolding — `envi-geo/envi-store/envi-service` Cargo.toml

**Analog (pure crate):** `crates/envi-engine/Cargo.toml` (17 lines, whole file):

```toml
[package]
name = "envi-engine"
description = "ENVI Nord2000 sound-propagation engine — pure math, zero I/O dependencies"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

# Boundary rule (01-RESEARCH "Architectural Responsibility Map"): this crate
# carries ZERO I/O dependencies. All file parsing lives in envi-harness.
[dependencies]
ndarray = "0.17"
num-complex = "0.4"
thiserror = "2"

[dev-dependencies]
approx = "0.5"
```

**Analog (I/O crate):** `crates/envi-harness/Cargo.toml` (lines 1-24):

```toml
[package]
name = "envi-harness"
description = "ENVI validation harness — FORCE/TOML case loading, comparison, reporting (all I/O lives here)"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
envi-engine = { path = "../envi-engine" }
calamine = "0.36"
serde = { version = "1", features = ["derive"] }
...
```

**Conventions to copy exactly:**
- `version.workspace = true` / `edition.workspace = true` / `rust-version.workspace = true` — never inline these three.
- A one-line `description` stating the crate's boundary rule ("all I/O lives here" style).
- A **boundary-rule comment above `[dependencies]`** explaining what may/may not enter (see both analogs). For `envi-geo`: "pure Rust CRS boundary — proj4rs + thiserror only, no I/O, no serde". For `envi-store`: "the serde quarantine seam — engine types are twinned here, never serde-derived".
- Path deps use relative `{ path = "../envi-engine" }`.
- There are **NO `[lints]` tables** anywhere in this workspace — do not introduce one. Lint policy is `#![deny(unsafe_code)]` in the crate root + the `cargo clippy --all-targets -- -D warnings` gate.
- `envi-service` gets a default `[[bin]]`-by-convention `src/main.rs`; the harness's explicit `[[test]] name = "force" harness = false` block (lines 26-31) is the precedent for non-default target tables if ever needed (not needed in Phase 6).

### 2. Workspace wiring — root `Cargo.toml` (verify-only)

**Whole file (9 lines):**

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
```

- `members = ["crates/*"]` globs — **the three new crates join automatically, zero root edits required** (matches CONTEXT.md).
- There is **no `[workspace.dependencies]` table**. House style is per-crate inline versions (`serde = { version = "1", ... }`). Do not add a workspace-deps table for three crates; follow the existing per-crate style so the diff surface stays minimal. (Planner may note this as an accepted style choice.)

### 3. Crate-root `lib.rs` — header + `#![deny(unsafe_code)]` placement

**Analog:** `crates/envi-engine/src/lib.rs` (whole file, 28 lines):

```rust
//! # envi-engine
//!
//! Pure-math core of the ENVI Nord2000 sound-propagation engine.
//!
//! This crate implements the Nord2000 method (AV 1106/07 rev. 4) as pure
//! functions over `f64` / `Complex<f64>` values. It performs **no I/O**: no
//! file parsing, no network, no environment access. Case loading, reference
//! comparison, and reporting live in the sibling `envi-harness` crate — that
//! quarantine is what makes "test harness before propagation code" an
//! architectural property rather than a sequencing note.
//!
//! Numerics house rules (see `.planning/PROJECT.md`):
//! - `f64` throughout; no `f32` in engine code.
//! - Typed errors, never panics, on data-dependent paths.
//!
#![deny(unsafe_code)]

pub mod directivity;
pub mod forest;
...
```

**Shape to replicate in all three new crates:**
1. `//! # crate-name` title line.
2. One-paragraph purpose + **explicit boundary statement** (what lives here vs. the sibling crates — e.g. envi-geo: "the ONE reprojection seam (GEOX-04); no other crate may call proj4rs"; envi-store: "serde lives HERE, never in envi-engine").
3. House-rules bullet list where applicable.
4. `#![deny(unsafe_code)]` immediately after the doc block, before any `pub mod` (all three new crates carry it — no FFI anywhere in Phase 6).
5. Flat `pub mod` list.

Compare `crates/envi-harness/src/lib.rs` lines 1-6 for the I/O-side variant ("**All I/O lives here**: ...").

### 4. Module I/O header convention (CLAUDE.md doc contract)

Every non-trivial module opens with a `//!` block: **title with spec/requirement refs, then named `#` sections for conventions, deviations, and pitfalls.** Two canonical examples:

**Analog A (engine module):** `crates/envi-engine/src/propagation/ground.rs` lines 1-19:

```rust
//! Ground reflection-coefficient chain (AV 1106/07 §5.6, Eqs. 57–77).
//!
//! Delany–Bazley impedance `Ẑ_G` (Eq. 57) → plane-wave `Γ̂_p` (Eq. 59) →
//! boundary-loss factor `Ê(ρ̂)` (Eq. 60) → spherical-wave reflection coefficient
//! `Q̂` (Eq. 58); plus the incoherent reflection coefficient `ρᵢ` (Eqs. 75–76).
//!
//! # Convention
//!
//! Nord2000-native: time e^{−jωt}, impedance **Im > 0** (do NOT flip the sign
//! here — ...). See [`super::special`].
//!
//! # Deviation from the plan interface block
//!
//! [`ground_impedance`] returns `Result` (not a bare `Complex<f64>`): σ crosses
//! from untrusted case-file data into the complex numerics, so σ ≤ 0 / non-finite
//! is rejected with a typed error rather than producing NaN/Inf ...
```

**Analog B (I/O-side module):** `crates/envi-harness/src/weather/mod.rs` lines 1-27 — note the explicit `# I/O quarantine (D-15)` section naming which crate owns what, and the `# ⚠️ ... [ASSUMED]` banner for documented deviations.

**Apply to new modules with these mandatory sections:**
- `envi-geo/src/transform.rs`: `# Convention` — "proj4rs longlat is RADIANS; converted inside this module and ONLY here" (RESEARCH Pitfall 1), plus the SC3 degree-magnitude rejection contract.
- `envi-geo/src/crs.rs`: `# Deviation` — the documented Norway/Svalbard zone-exception skip (RESEARCH Pattern 1 zone-selection note).
- `envi-store/src/dto.rs`: `# Quarantine` — "serde lives HERE, never in envi-engine (Anti-Pattern 1); DTO ⇄ engine via From/TryFrom".
- `envi-store/src/hash.rs`: `# Frozen encoding` — canonical `f64::to_bits` LE bytes, version prefix, "conditioning is NEVER hashed" (mirrors the "frozen composition order" doc style of `tensor.rs::compose_gain`, lines 366-384).
- `envi-service/src/jobs.rs`: `# Anti-Pattern 5` — dedicated `std::thread`, never `spawn_blocking`.

### 5. Error handling — the three error-enum archetypes

**Archetype A — pure-crate domain error** (for `GeoError` in envi-geo):
**Analog:** `crates/envi-engine/src/scene.rs::SceneError` lines 31-66:

```rust
/// Errors from constructing scene domain types out of untrusted case data.
///
/// Every malformed input yields one of these — the scene constructors never
/// panic on data (threat T-01-05, DoS via malformed-input panic).
#[derive(Debug, Error, PartialEq)]
pub enum SceneError {
    /// A terrain profile must contain at least one point.
    #[error("terrain profile is empty")]
    EmptyProfile,
    /// Profile X coordinates must be strictly ascending along the cut plane.
    #[error("terrain profile X not strictly ascending at point {index} (x = {x} after {prev_x})")]
    NonAscendingX {
        /// Offending point index (0-based).
        index: usize,
        /// Previous X value.
        prev_x: f64,
        /// Offending X value.
        x: f64,
    },
    ...
    /// A coordinate or segment property was NaN or infinite.
    #[error("non-finite value: {what}")]
    NonFinite { what: String },
}
```

Conventions: `#[derive(Debug, Error, PartialEq)]` (PartialEq so tests can `assert_eq!` on variants); **struct variants carrying the offending values** with a rustdoc line per field; error messages state got/expected. `GeoError` copies this shape (`LonLatOutOfRange { lon, lat }`, `DegreeMagnitudeSceneCoord { x, y }`, a `Proj` wrapper variant).

**Archetype B — I/O-crate error wrapping sources** (for `StoreError` in envi-store):
**Analog:** `crates/envi-harness/src/cases/mod.rs::CaseLoadError` lines 46-93:

```rust
/// Typed load error for untrusted case input (ASVS V5 posture, T-01-01):
/// every malformed input yields one of these — never a panic.
#[derive(Debug, Error)]
pub enum CaseLoadError {
    /// Filesystem error reading a case file.
    #[error("I/O error reading {path}: {source}")]
    Io {
        /// Offending path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// TOML syntax / schema error.
    #[error("TOML parse error in {path}: {message}")]
    TomlParse { path: PathBuf, message: String },
    ...
}
```

Conventions: no `PartialEq` when wrapping `std::io::Error`; `#[source]` on the underlying error; **the offending `PathBuf` always travels in the variant**. `StoreError` copies this (`Io { path, source }`, `Json { path, message }`, `GeoJson {..}`, `NotFound { project_id }`, `BadBandCount(usize)`...).

**Archetype C — validation-at-the-boundary error** (for DTO validation):
**Analog:** `crates/envi-engine/src/tensor.rs::SinkError` lines 62-131 — every dimension/shape mismatch is its own variant with `expected`/`got` fields; non-finite values are rejected with `NonFinite { what }` **before touching memory** (lines 228-268). Spectrum DTOs replicate the length + finiteness checks.

**`ApiError` (envi-service) has no analog** — build per RESEARCH (`error.rs`: enum → `IntoResponse` mapping status + JSON body). Reuse Archetype A's got/expected field style so the 409 body carries `expected`/`got` hashes.

**House rule visible in all three analogs:** typed `Result` on every data-dependent path, `never panics on data`; `debug_assert!` only for programmer invariants (`scene.rs` line 289); `#[must_use]` on pure constructors/getters.

### 6. The engine types the DTO mirror twins — EXACT shapes (verify-only, from `crates/envi-engine/src/scene.rs`)

| Engine type | Fields (visibility!) | DTO-conversion note |
|---|---|---|
| `CrsInfo` (l. 70-90) | `pub label: String`; ctor `local_metric()`; `Default` | Phase 6 pins the real UTM CRS in project.json; when converting DTO→engine, set `label` from `envi-geo::ProjectCrs` (e.g. `"utm-31n"`). Descriptive only. |
| `BandSpectrum` (l. 97-123) | **private** `values_db: [f64; N_BANDS]` | No public field access. Build via `BandSpectrum::from_values([f64; 105])`, read via `.as_slice() -> &[f64]`. `Vec<f64>` → array needs `TryFrom` (RESEARCH Pattern 2 shows the exact impl). |
| `SubSource` (l. 130-136) | `pub position: [f64; 3]`, `pub spectrum: BandSpectrum` | All-pub, direct twin. |
| `Source` (l. 139-143) | `pub sub_sources: Vec<SubSource>` | All-pub. |
| `Receiver` (l. 146-150) | `pub position: [f64; 3]` | All-pub. |
| `Barrier` (l. 158-176) | `pub top_edge: Vec<[f64; 3]>`, `pub thickness_m: Option<f64>` (`None` = thin) | `Option` semantics are load-bearing (thin vs thick screen) — DTO must preserve `null` ≠ `0.0`. |
| `Building` (l. 179-185) | `pub footprint: Vec<[f64; 2]>`, `pub eaves_height_m: f64` | All-pub. |
| `GroundSegment` (l. 188-194) | `pub flow_resistivity: f64` (kNs·m⁻⁴), `pub roughness: f64` (m) — **Copy** | Impedance class chars A–H map via `scene::impedance_class(char) -> Option<f64>` (l. 328-341; **B = 31.5**). The GeoJSON `properties` carry the class letter; conversion resolves it to σ. |
| `TerrainProfile` (l. 203-298) | **private** `points: Vec<[f64; 2]>`, `segments: Vec<GroundSegment>` | MUST construct via `TerrainProfile::new(points, segments) -> Result<Self, SceneError>` (validates non-empty, strictly-ascending X, N−1 segments, finite). DTO→engine is inherently `TryFrom`. Read via `.points()` / `.segments()`. `endpoints(h_s, h_r)` encodes the hSv/hRv convention (source above FIRST point, receiver above LAST). |
| `Scene` (l. 301-315) | all pub: `crs: CrsInfo`, `sources: Vec<Source>`, `receivers: Vec<Receiver>`, `barriers: Vec<Barrier>`, `buildings: Vec<Building>`, `terrain: Vec<TerrainProfile>` | Top-level twin. Note: engine `Scene` has **no forest/ground-zone/calc-area kinds** — those GeoJSON kinds are persisted-but-not-engine-mapped in Phase 6 (CONTEXT: "unknown-to-engine kinds are persisted, not dropped"). |

Coordinate convention (scene.rs header, l. 3-10): projected metric CRS, meters, Z-up, positions `[x, y, z]` — the `envi-geo::SceneXY` target space.

### 7. Freq axis for `GET /api/v1/meta/freq-axis` (from `crates/envi-engine/src/freq.rs`)

- `pub const N_BANDS: usize = 105` (l. 36); `pub const N_THIRD_OCT: usize = 27` (l. 39).
- `pub const NOMINAL_THIRD_OCT: [f64; 27]` (l. 49-53) — display labels ONLY.
- `pub struct FreqAxis { pub centres: [f64; N_BANDS] }` (l. 67-70); `third_octave_pick(i)` = `centres[i * 4]` (l. 90-96).
- `pub static FREQ_AXIS: LazyLock<FreqAxis>` (l. 106) — the service DTO is built from `FREQ_AXIS.centres.to_vec()` at runtime, never hard-coded (RESEARCH Pitfall 9; test pins `centres_hz[64] == 1000.0` exactly — bit-exact per freq.rs test l. 120).
- `BandIdx(pub usize)` newtype exists (l. 60) but need not cross the wire — wire arrays are dense `[105]` by position.
- Module header (l. 26-31) states the binding pitfall verbatim: "never compare nominal frequencies as floats … `f == 31.5` anywhere is a bug." The wire contract (band-index keys, no Hz keys) is this rule expressed as an API.

### 8. Forward-compat seams — manifest + stub DTOs must mirror these (verify-only)

**`crates/envi-engine/src/tensor.rs`:**

```rust
// l. 136-142
pub struct TensorPair {
    pub h_coh: Array3<Complex<f64>>,     // [sub_source, receiver, freq], row-major
    pub p_incoh_abs: Array3<f64>,        // same shape, real, ≥ 0
}
// l. 166-181
pub trait TensorSink {
    fn put_chunk(
        &mut self,
        r_offset: usize,
        h_coh: ArrayView3<'_, Complex<f64>>,      // [n_sub, chunk_len, N_BANDS]
        p_incoh_abs: ArrayView3<'_, f64>,
    ) -> Result<(), SinkError>;
}
// l. 48, 54
pub const BYTES_PER_CELL_PAIR: usize = 16 + 8;                  // 24 B/cell across the pair
pub const DEFAULT_TENSOR_BUDGET_BYTES: usize = 256 * 1024 * 1024;
// chunk_receivers = floor(budget / (n_sub · N_BANDS · 24))     — doc comment l. 50-53
```

Manifest consequences: `dims: [S, R, 105]`, chunking **on the receiver axis** (chunk key = `r_offset`), two channel dirs (`tensor/` = complex 16 B cells, `pincoh/` = real 8 B cells). Record `chunk_receivers` in the manifest so Phase 9 file naming is already decided (RESEARCH Pattern 5).

**`crates/envi-engine/src/solver.rs::SolveJob`** (l. 62-121) fields: `sub_source: usize`, `receiver: usize`, `profile: &TerrainProfile`, `src: [f64;3]`, `rcv: [f64;3]`, `atmosphere: &Atmosphere`, `coh: &CoherenceInputs`, `axis: &FreqAxis`, `weather: Option<&SoundSpeedProfile>`, `directivity_gain_db: Option<[f64; N_BANDS]>`, `directivity_phase_rad: Option<[f64; N_BANDS]>`, `forest: Option<ForestCrossing>`, `isolation: Option<&IsolationSpectrum>`.

**The load-bearing confirmation for the hash contract:** conditioning (gain/delay/filter/mute) appears **nowhere** in `SolveJob` — it enters only at readout via `tensor::compose_gain(l_w_db, filter, delay_s, axis)` (l. 385-425). The engine's own structure proves conditioning is a readout parameter → it must NEVER enter `tensor_hash` (D-07). Geometry/met/receivers (which DO construct `SolveJob`s in Phase 9) are exactly the hash inputs.

`solve()` signature (l. 139-147): jobs receiver-major, `chunk_receivers` chunking, streams into `&mut dyn TensorSink` — the Phase-9 real compute the stub job's shape must not contradict.

### 9. Binary entry point — `envi-service/src/main.rs`

**Analog:** `crates/envi-harness/src/main.rs` (88 lines):

```rust
//! `envi-harness` CLI — human-readable per-case outcome report.
//!
//! Usage: `cargo run -p envi-harness -- report`
...
/// Workspace root, resolved from this crate's manifest dir so the binary finds
/// `refs/` and `cases/` regardless of the invocation working directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/envi-harness has a workspace root two levels up")
        .to_path_buf()
}

fn main() -> std::process::ExitCode {
    let cmd = std::env::args().nth(1).unwrap_or_else(|| "report".to_string());
    if cmd != "report" {
        eprintln!("usage: envi-harness report");
        return std::process::ExitCode::from(2);
    }
    ...
}
```

Copy: the `//! Usage:` doc header, the `workspace_root()` `CARGO_MANIFEST_DIR`-relative path-resolution idiom (relevant for locating `web/dist` and the default `projects/` root in dev runs), and explicit non-zero exit on failure (`main() -> ExitCode` here; the async service uses `main() -> Result<..>` returning `Err` on self-check failure per D-08 — same refuse-to-start semantics).

### 10. Test conventions

**(a) Unit tests: `#[cfg(test)] mod tests` at the bottom of every module** — see `scene.rs` l. 343-450, `freq.rs` l. 108-155, `tensor.rs` l. 572-768. Conventions:
- Descriptive snake_case names that state the contract: `terrain_profile_rejects_malformed_input_with_typed_errors`, `two_identical_incoherent_sources_add_three_db_not_six`.
- Error-path tests use `matches!(err, SceneError::NonAscendingX { .. })` with `"got {err:?}"` messages.
- Float compares via `approx::assert_relative_eq!` (dev-dep `approx = "0.5"`); **bit-exact contracts use `.to_bits()` equality** (`freq.rs` l. 139-142) — the pattern for "phase-free path is bit-identical" style assertions.
- New-crate equivalents: `envi-geo` `to_wgs84_rejects_degree_magnitude_input` (matches! on `GeoError`), `envi-store` `band_spectrum_dto_validates_length`, `tensor_hash_ignores_conditioning...`.

**(b) Integration tests: `crates/<crate>/tests/*.rs`** — the harness has 19 of them. `envi-service` contract tests (oneshot router, 409, SSE) go in `crates/envi-service/tests/` following this placement.

**(c) Oracle-fixture pattern (for the CRS pyproj fixture)** — **Analog:** `crates/envi-harness/tests/oracle_ground.rs` (whole file, 105 lines) + `tools/nord2000_oracle/gen_ground_fixtures.py`:

```rust
//! Cross-implementation oracle test (02-RESEARCH Pattern 4).
//!
//! Loads the committed `tests/fixtures/oracle/ground_w_qhat.toml` — generated by
//! `tools/nord2000_oracle/gen_ground_fixtures.py` from `scipy.special.wofz` — and
//! asserts ... Python/scipy are NOT needed at test time; the TOML is the
//! committed data.

#[derive(Deserialize)]
struct Fixtures { meta: Meta, w: Vec<WRow>, qhat: Vec<QRow> }

#[derive(Deserialize)]
struct Meta { w_tol_rel: f64, qhat_tol_rel: f64 }

fn load() -> Fixtures {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/oracle/ground_w_qhat.toml");
    let text = std::fs::read_to_string(path).expect("oracle fixture TOML must exist");
    toml::from_str(&text).expect("oracle fixture TOML must parse")
}
```

And the generator header convention (`gen_ground_fixtures.py` l. 1-10): docstring naming the output path, "Regeneration is operator-driven", "Python/scipy are NOT build dependencies", and **a sha256 provenance hash of the oracle source recorded in the fixture header**. `tools/crs_oracle/gen_utm.py` copies this shape (pyproj instead of scipy); the committed fixture goes to `crates/envi-geo/tests/fixtures/oracle/utm_landmarks.toml`. Note: `envi-geo` must NOT gain serde/toml as runtime deps — but `[dev-dependencies] serde/toml` for the fixture test is fine (matches how the harness consumes fixtures; only the harness has them as runtime deps).

**(d) Honest fail-soft (Outcome/capability ethos)** — **Analog:** `crates/envi-harness/src/lib.rs::Outcome` (l. 28-42) + `run_case` gating (l. 50-59) + `tests/force.rs` (Skipped → ignored `Trial` with the requires-list as the visible reason, l. 64-71). Phase-6 translation: the stub compute must be *honestly* stubbed — `recondition` returns a canned spectrum clearly labeled in the DTO/manifest (e.g. `"stub": true` or provenance field), never anything that could read as a real acoustic result. Mirrors "capability-gated honesty" from CONTEXT.md.

### 11. Quarantine gates — where they live and how they run

Neither gate is a committed test file; both are **documented commands** enforced at plan/gate time:

- **Dependency quarantine:** `cargo tree -p envi-engine` must print exactly `ndarray`, `num-complex`, `thiserror` (+ transitives). Stated in `.claude/CLAUDE.md` (codebase-as-built §) and `crates/envi-harness/Cargo.toml` l. 19-20 comment. Adding three new crates does not touch it — but the planner must include the check as an explicit verification step (precedent: `04-01-PLAN.md` embeds it in `<automated>` checks).
- **conj grep gate:** root `README.md` l. 35-38: `grep -rh '\.conj()' crates/envi-engine/src/propagation/` returns **0**. Phase 6 touches no propagation code → trivially green; include as a verification no-op.
- Planner precedent for embedding both (from `04-01-PLAN.md` l. 164): `grep -rl --include='*.rs' 'conj()' crates/envi-engine/src/propagation/ | grep -c . | grep -qx 0 && echo CONJ_QUARANTINE_OK`.

## Shared Patterns

### Typed-error / never-panic-on-data posture
**Source:** `scene.rs` l. 31-34 doc ("the scene constructors never panic on data (threat T-01-05)"), `tensor.rs` l. 56-61, `cases/mod.rs` l. 46-47.
**Apply to:** every constructor/parser in all three new crates. Untrusted input (HTTP bodies, files on disk, coordinates) always crosses a validating boundary returning a typed error. `Path<Uuid>` extractors are this posture applied to URLs.

### Rustdoc completeness
**Source:** every pub item AND every struct/enum field in `scene.rs`, `tensor.rs`, `cases/mod.rs` carries a doc comment (even error-variant fields — see `SceneError::NonAscendingX` fields). `#[must_use]` on pure fns.
**Apply to:** all new pub items. This is the standing style the clippy gate rides on.

### Requirement/threat traceability in comments
**Source:** comments cite requirement IDs and decisions inline — `tensor.rs` l. 21 "(OUT-03)", l. 32 "(OUT-06)", `ground.rs` l. 18 "(threat T-02-01)".
**Apply to:** new code cites SVC-xx/GEOX-04/D-xx the same way (e.g. the degree-magnitude guard cites SC3; the 409 path cites D-07).

### Frozen-contract documentation style
**Source:** `tensor.rs::compose_gain` l. 366-384 — "The three factors multiply in this frozen order, ONCE per band … written explicitly, never via `.conj()`".
**Apply to:** `hash.rs` (frozen canonical byte encoding, version-prefixed) and the wire DTO modules (frozen request/response shapes; extend via `#[serde(default)]`, never break).

## No Analog Found

Files with no codebase analog — the planner should build these from 06-RESEARCH.md's verified patterns (all pinned there with doc sources):

| File | Role | Data Flow | Reason / Pattern source |
|------|------|-----------|--------------------------|
| `envi-service/src/api/*` (router + handlers) | controller | request-response | First HTTP code in the repo — RESEARCH §Code Examples (axum 0.8 `/{id}` syntax, `Router::nest`, ServeDir fallback) |
| `envi-service/src/jobs.rs` (registry + SSE) | service | event-driven | First async/event code — RESEARCH Pattern 4 (std::thread worker + CancellationToken + watch → WatchStream → SSE) |
| `envi-service/src/state.rs` | provider | request-response | RESEARCH §Architecture (AppState, `tokio::sync::RwLock<HashMap>` registry) |
| `envi-service/src/selfcheck.rs` | utility | — | RESEARCH Pattern 6 (Dam Square landmark round-trip, refuse-to-start) |
| `envi-store/src/project_dir.rs` atomic write | service | file-I/O | RESEARCH Pattern 3 (`NamedTempFile::new_in(dir)` + `sync_all` + `persist`) — error-enum shape from Archetype B above |
| `envi-store/src/geojson.rs` | transform | file-I/O | `geojson` crate 1.0 + `properties.kind` vocabulary (ARCHITECTURE.md locked list); mapping ethos from `scene_build.rs` |
| `web/dist/index.html` | static asset | — | RESEARCH Open Q3 (dependency-free placeholder that fetches `/api/v1/meta/freq-axis`) |

## Metadata

**Analog search scope:** `crates/envi-engine/src/**`, `crates/envi-harness/{src,tests}/**`, `tools/nord2000_oracle/`, root `Cargo.toml`, root `README.md`
**Files scanned:** 18 read in full or targeted-section (scene.rs, freq.rs, tensor.rs, solver.rs 1-230, transfer refs, both Cargo.tomls, root Cargo.toml, engine lib.rs, harness lib.rs, harness main.rs, cases/mod.rs 1-100, weather/mod.rs 1-40, ground.rs 1-60, tests/force.rs, tests/oracle_ground.rs, gen_ground_fixtures.py header, README.md 20-79)
**Pattern extraction date:** 2026-07-09

**Flags for the planner:**
1. **`crates/README.md` does not exist.** CLAUDE.md's doc contract names it ("Module I/O headers, `crates/README.md`, and root `README.md` reflect any new feature"). Phase 6 must create it (crate table for all four crates) and extend the root `README.md` workspace table (l. 18-22) + add the service run command.
2. **No `[workspace.dependencies]` table exists** — per-crate inline versions are the house style; don't introduce one silently.
3. **No `[lints]` tables exist** — lint policy is crate-root attributes + the clippy `-D warnings` gate.
4. **Both quarantine gates are commands, not committed tests** — embed them in plan `<automated>` verification (04-01-PLAN precedent).
5. `BandSpectrum` and `TerrainProfile` have **private fields + validating constructors** — DTO→engine conversions are necessarily `TryFrom` through `from_values`/`new()`, exactly as RESEARCH Pattern 2 sketches.
