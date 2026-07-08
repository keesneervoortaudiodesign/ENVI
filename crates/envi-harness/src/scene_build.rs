//! `CaseDefinition` → engine [`Scene`] conversion (GEO-02).
//!
//! This is the trust boundary where untrusted parsed case data (profile rows,
//! heights, positions) crosses into the engine's domain constructors. The
//! FORCE straight-road branch applies the lane and hSv/hRv conventions that
//! are the phase's biggest off-by-metres traps (01-RESEARCH Pitfall 5):
//!
//! - The **source line** sits at `x = 2.5 m` from the road centre line
//!   (vehicles in the middle of the nearest 5 m lane), while the terrain
//!   profile starts at `x = 3.25 m` — so the case-1 horizontal source→receiver
//!   distance is **97.5 m, NOT 100 m**.
//! - **Source height** is measured above the FIRST profile point, **receiver
//!   height** above the LAST (via [`TerrainProfile::endpoints`]).
//!
//! Synthetic (free-field / geometry) cases map their TOML positions literally —
//! no lane convention is applied to non-FORCE kinds.

use anyhow::{Context, anyhow};

use envi_engine::freq::N_BANDS;
use envi_engine::scene::{
    BandSpectrum, Building, CrsInfo, GroundSegment, Receiver, Scene, Source, SubSource,
    TerrainProfile,
};

use crate::cases::{CaseDefinition, CaseKind, SourceSpectrum};
use crate::emission::{RoadCategory, RoadSource, RoadSurface};

/// Materialize a [`SourceSpectrum`] spec into the engine's [`BandSpectrum`]
/// (the point sub-source's per-1/12-octave `L_W`, SRC-01).
fn band_spectrum(spec: SourceSpectrum) -> BandSpectrum {
    match spec {
        SourceSpectrum::Unit => BandSpectrum::uniform(0.0),
        SourceSpectrum::Uniform(db) => BandSpectrum::uniform(db),
        SourceSpectrum::Ramp {
            base_db,
            slope_db_per_band,
        } => {
            let values: [f64; N_BANDS] =
                std::array::from_fn(|i| base_db + slope_db_per_band * i as f64);
            BandSpectrum::from_values(values)
        }
    }
}

/// Vehicle **centre-line** offset from the road centre line for FORCE
/// straight-road cases: vehicles drive in the middle of the nearest 5 m lane →
/// `x = 2.5 m`.
///
/// # Supersedes the Phase-1 placeholder (04-02, Pitfall 1)
///
/// Phase 1 froze `FORCE_LANE_X_M = 2.5` as the **source line** and a single
/// height-0 placeholder sub-source (`FORCE_PLACEHOLDER_SOURCE_H_M = 0.0`),
/// giving a 97.5 m horizontal distance. That is now **superseded**: the
/// Nord2000 sub-sources sit [`emission::SOURCE_OFFSET_M`] = 1 m from the vehicle
/// centre line **toward the receiver** (AV 1171/06 §3.1.1) at heights 0.01 /
/// 0.30 / 0.75 m — so the FORCE source line is `2.5 + 1.0 = 3.5 m` and the
/// case-1 horizontal distance is **96.5 m**, not 97.5 m. The single placeholder
/// sub-source is replaced by the [`emission::RoadSource`] expansion.
const FORCE_LANE_CENTRE_X_M: f64 = 2.5;

/// Default FORCE straight-road traffic for scene building: Category-1 light
/// vehicles at 80 km/h on DAC-12 (cases 17/18 are cat-2/3 — category detection
/// from case metadata is a 04-03 concern). Air temperature comes from the case.
const FORCE_DEFAULT_SPEED_KMH: f64 = 80.0;

/// Build the canonical semantic [`Scene`] for a case.
///
/// # Errors
///
/// Returns an error if the FORCE terrain profile is malformed (propagated from
/// [`TerrainProfile::new`]), a FORCE case is missing its receiver height, a
/// synthetic case is missing source/receiver positions, or the case kind has
/// no Phase 1 scene builder.
pub fn build_scene(case: &CaseDefinition) -> anyhow::Result<Scene> {
    match case.kind {
        CaseKind::ForceStraightRoad => build_force_straight_road(case),
        CaseKind::FreeField | CaseKind::Geometry => build_synthetic(case),
        other => Err(anyhow!(
            "build_scene not implemented for {other:?} (case {})",
            case.id
        )),
    }
}

/// FORCE straight-road → Scene, applying the lane / hSv / hRv conventions.
fn build_force_straight_road(case: &CaseDefinition) -> anyhow::Result<Scene> {
    let rows = &case.terrain_profile;
    if rows.is_empty() {
        return Err(anyhow!(
            "FORCE case {} has an empty terrain profile",
            case.id
        ));
    }

    // Points are (x, z); x is distance from the road centre line — the SAME
    // frame as the source line. N rows → N−1 segments, each taking the flow
    // resistivity / roughness of the row that STARTS it. Case 1 is actually
    // MIXED impedance in the authoritative .xls (road strip σ=20000 at x=3.25,
    // then grass σ=12.5 at x=5), so this row→segment rule IS observable and is
    // verified by the case-1 test — the plan's "all class A" assumption was
    // wrong (corrected against the real data, Pitfall 1).
    let points: Vec<[f64; 2]> = rows.iter().map(|r| [r.x_m, r.z_m]).collect();
    let segments: Vec<GroundSegment> = rows
        .windows(2)
        .map(|w| GroundSegment {
            flow_resistivity: w[0].flow_resistivity_kns_m4,
            roughness: w[0].roughness_m,
        })
        .collect();
    let terrain = TerrainProfile::new(points, segments)
        .with_context(|| format!("building terrain profile for case {}", case.id))?;

    let h_r = case
        .propagation
        .hr_m
        .ok_or_else(|| anyhow!("FORCE case {} is missing the receiver height hr", case.id))?;

    // hSv/hRv: receiver Z above the LAST profile point (endpoints encodes the
    // convention in one place). The ground elevation the source heights sit
    // above is the FIRST profile point's Z (source-side ground).
    let (src_xz, rcv_xz) = terrain.endpoints(0.0, h_r);
    let ground_z = src_xz[1];
    let receiver_pos = [rcv_xz[0], 0.0, rcv_xz[1]];

    // Road emission expansion (04-02): the single Phase-1 placeholder sub-source
    // is superseded by the Nord2000 height sub-sources at x = 3.5 m (lane centre
    // 2.5 m + 1 m toward the receiver, Pitfall 1). The receiver is at large +x,
    // so "toward the receiver" is +x.
    let road = RoadSource {
        category: RoadCategory::Light,
        speed_kmh: FORCE_DEFAULT_SPEED_KMH,
        surface: RoadSurface::Dac12,
        temperature_c: case.propagation.t0_c.unwrap_or(15.0),
    };
    let sub_sources: Vec<SubSource> = road
        .expand([FORCE_LANE_CENTRE_X_M, 0.0], [1.0, 0.0], ground_z)
        .into_iter()
        .map(|rs| rs.sub_source)
        .collect();

    Ok(Scene {
        crs: CrsInfo::local_metric(),
        sources: vec![Source { sub_sources }],
        receivers: vec![Receiver {
            position: receiver_pos,
        }],
        barriers: Vec::new(),
        buildings: Vec::<Building>::new(),
        terrain: vec![terrain],
    })
}

/// Synthetic (free-field / geometry) case → Scene: TOML positions verbatim.
fn build_synthetic(case: &CaseDefinition) -> anyhow::Result<Scene> {
    let source_pos = case
        .source_position
        .ok_or_else(|| anyhow!("synthetic case {} is missing a source position", case.id))?;
    let receiver_pos = case
        .receiver_position
        .ok_or_else(|| anyhow!("synthetic case {} is missing a receiver position", case.id))?;

    Ok(Scene {
        crs: CrsInfo::local_metric(),
        sources: vec![Source {
            sub_sources: vec![SubSource {
                position: source_pos,
                spectrum: band_spectrum(case.source_spectrum),
            }],
        }],
        receivers: vec![Receiver {
            position: receiver_pos,
        }],
        barriers: Vec::new(),
        buildings: Vec::new(),
        terrain: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{CaseKind, PropagationParams, ReferenceVersion, SyntheticExpected};
    use approx::assert_relative_eq;
    use envi_engine::geometry::PathGeometry;
    use std::path::{Path, PathBuf};

    fn straight_road_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join("refs")
            .join("TestStraightRoad.xls")
    }

    #[test]
    fn force_case_1_applies_lane_and_height_conventions() {
        // Auto-skips (green) when refs/ is not fetched — like the xls loader
        // anchor test; the force runner surfaces the real skip.
        let path = straight_road_path();
        if !path.is_file() {
            eprintln!("SKIP: {} not fetched — run refs/fetch.sh", path.display());
            return;
        }
        let cases = crate::cases::xls::load_straight_road(&path).expect("workbook must open");
        let case1 = cases[0].case.as_ref().expect("case 1 must load");

        let scene = build_scene(case1).expect("scene must build");
        let terrain = &scene.terrain[0];

        // First profile point is 3.25 m from the road centre line.
        assert_relative_eq!(terrain.points()[0][0], 3.25, epsilon = 1e-12);

        // Case 1 is MIXED impedance in the authoritative .xls (the plan's
        // must_have "every segment = 12.5" was based on a wrong assumption —
        // corrected against the real data, Pitfall 1). Profile points/segments:
        //   x=3.25 (σ=20000, road) → x=5 (σ=12.5, grass) → x=100 (σ terminator).
        // The row→segment rule "each segment takes the flow resistivity of the
        // row that STARTS it" is now OBSERVABLE and verified here (resolving the
        // plan's flagged Phase-2 re-verification early):
        assert_eq!(terrain.points().len(), 3);
        assert_eq!(terrain.segments().len(), 2);
        // Road pavement strip (3.25–5 m) is class G = 20000 kNs·m⁻⁴.
        assert_relative_eq!(
            terrain.segments()[0].flow_resistivity,
            20000.0,
            epsilon = 1e-12
        );
        // Grass (5–100 m), the dominant ground, is class A = 12.5 kNs·m⁻⁴.
        assert_relative_eq!(
            terrain.segments()[1].flow_resistivity,
            12.5,
            epsilon = 1e-12
        );

        // Emission expansion (04-02): TWO sub-sources at the Nord2000 heights,
        // both on the source line x = 3.5 m (lane centre 2.5 m + 1 m toward the
        // receiver, Pitfall 1 — superseding the Phase-1 x=2.5/height-0 placeholder).
        assert_eq!(scene.sources[0].sub_sources.len(), 2);
        let low = scene.sources[0].sub_sources[0].position;
        let high = scene.sources[0].sub_sources[1].position;
        let receiver = scene.receivers[0].position;

        assert_relative_eq!(low[0], 3.5, epsilon = 1e-12);
        assert_relative_eq!(high[0], 3.5, epsilon = 1e-12);
        // Heights 0.01 m (rolling-low) and 0.30 m (propulsion-high) for cat-1.
        assert_relative_eq!(low[2], 0.01, epsilon = 1e-12);
        assert_relative_eq!(high[2], 0.30, epsilon = 1e-12);
        assert_relative_eq!(receiver[0], 100.0, epsilon = 1e-12);
        // Receiver height 1.5 m above the LAST (flat, z = 0) profile point.
        assert_relative_eq!(receiver[2], 1.5, epsilon = 1e-12);

        // THE superseded anchor: horizontal source→receiver distance is now
        // 96.5 m (100 − 3.5), NOT the Phase-1 97.5 m (Pitfall 1).
        let geom = PathGeometry::direct(low, receiver).unwrap();
        assert_relative_eq!(geom.horizontal_m, 96.5, max_relative = 1e-12);
    }

    #[test]
    fn synthetic_geometry_case_maps_positions_literally() {
        let case = CaseDefinition {
            id: "toml::geom".to_string(),
            name: "geom".to_string(),
            kind: CaseKind::Geometry,
            reference_version: ReferenceVersion::Analytic,
            description: "geometry".to_string(),
            source_position: Some([0.0, 0.0, 2.0]),
            source_spectrum: crate::cases::SourceSpectrum::Unit,
            receiver_position: Some([100.0, 100.0, 2.0]),
            propagation: PropagationParams::default(),
            terrain_profile: Vec::new(),
            reference_spectrum: None,
            expected: Some(SyntheticExpected {
                tolerance_db: 1e-9,
                bands: "geometry".to_string(),
                geometry: None,
                reference_bands: None,
            }),
        };
        let scene = build_scene(&case).expect("synthetic scene builds");
        // No lane convention: positions are verbatim.
        assert_eq!(scene.sources[0].sub_sources[0].position, [0.0, 0.0, 2.0]);
        assert_eq!(scene.receivers[0].position, [100.0, 100.0, 2.0]);
        assert!(scene.terrain.is_empty());
    }
}
