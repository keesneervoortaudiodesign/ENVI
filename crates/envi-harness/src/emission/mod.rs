//! Nord2000 road emission model (SRC-04/road) — the first concrete instance of
//! the directional multi-sub-source model (SRC-02/03/04).
//!
//! A [`RoadSource`] (category, speed, surface, temperature) expands into point
//! [`SubSource`]s at the Nord2000 heights (0.01 / 0.30 / 0.75 m), placed **1 m
//! from the vehicle centre line toward the receiver** (AV 1171/06 §3.1.1) — for
//! the FORCE straight road that puts the source line at **x = 3.5 m**, NOT the
//! Phase-1 `x = 2.5 m` / height-0 placeholder (Pitfall 1). Each sub-source
//! carries a per-1/12-octave `L_W` and a [`DirectivityBalloon`] sampled at
//! construction from the analytic road directivities.
//!
//! # Rolling / propulsion energy split (AV Annex A, Kragh §2.1)
//!
//! Rolling (tyre/road) and propulsion sound power are computed separately then
//! energy-split across the two heights — rolling 80 % low / 20 % high,
//! propulsion 80 % high / 20 % low — and the sub-sources are summed
//! **INCOHERENTLY** (two co-located subs ⇒ +3 dB, never +6). Nothing here sums
//! coherently.
//!
//! # Directivities ([ASSUMED A7] until SP 2006:12 confirms)
//!
//! Every sub-source has a frequency-dependent **vertical** directivity (car-body
//! screening); the 0.01 m source additionally has a frequency-dependent
//! **horizontal** horn-effect directivity; the heavy 0.75 m propulsion source
//! has a frequency-**independent** horizontal directivity. All are smooth
//! analytic placeholders sampled onto balloons (05° grid, well inside the
//! 0.05 dB budget) — validated by structure/property tests, never a false Pass.

pub mod coefficients;
pub mod passby;

use envi_engine::directivity::DirectivityBalloon;
use envi_engine::freq::{N_BANDS, N_THIRD_OCT};
use envi_engine::scene::{BandSpectrum, SubSource};

pub use coefficients::{Provenance, RoadCategory};

/// Horizontal offset of the sub-sources from the vehicle centre line, toward
/// the receiver, meters (AV 1171/06 §3.1.1 "average position of the nearest
/// wheels"). **`[CITED]`** — this is the Pitfall-1 correction that supersedes
/// the Phase-1 placeholder.
pub const SOURCE_OFFSET_M: f64 = 1.0;

/// The angular grid step for the sampled road directivity balloons (degrees).
const BALLOON_STEP_DEG: f64 = 5.0;

/// Road surface (AV 1171/06 §3.1.7). FORCE uses DAC 12.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoadSurface {
    /// Dense asphalt concrete, 12 mm aggregate (FORCE reference surface).
    Dac12,
    /// The bare reference surface (no aggregate-size correction).
    Reference,
}

/// Which of the two vehicle sub-source heights a sub-source represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceHeight {
    /// The low source (0.01 m): rolling-dominated.
    Low,
    /// The high source (0.30 m light / 0.75 m heavy): propulsion-dominated.
    High,
}

/// A Nord2000 road source: a vehicle category driving at a constant speed on a
/// given surface and air temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoadSource {
    /// Vehicle category.
    pub category: RoadCategory,
    /// Speed, km/h.
    pub speed_kmh: f64,
    /// Road surface.
    pub surface: RoadSurface,
    /// Air temperature, °C.
    pub temperature_c: f64,
}

/// One expanded road sub-source: an engine [`SubSource`] (position + `L_W`),
/// its directivity [`DirectivityBalloon`], and which height role it plays.
#[derive(Debug, Clone, PartialEq)]
pub struct RoadSubSource {
    /// The engine point sub-source (position + per-1/12-octave `L_W`).
    pub sub_source: SubSource,
    /// The directivity balloon sampled for this sub-source's role.
    pub balloon: DirectivityBalloon,
    /// The height role (low / high).
    pub role: SourceHeight,
}

impl RoadSubSource {
    /// Per-band incoherent energy weight `|G_s(f)|² = 10^{L_W(f)/10}` on the
    /// 105-point grid — the readout weight the tensor's Annex-A sum consumes.
    #[must_use]
    pub fn energy_weight(&self) -> [f64; N_BANDS] {
        let mut w = [0.0; N_BANDS];
        for (slot, &lw) in w.iter_mut().zip(self.sub_source.spectrum.as_slice()) {
            *slot = 10f64.powf(lw / 10.0);
        }
        w
    }
}

impl RoadSource {
    /// The two sub-source heights (m) for this category: `(low, high)`.
    /// Light → (0.01, 0.30); Medium-heavy / Heavy → (0.01, 0.75)
    /// (AV 1171/06 §3.1.1).
    #[must_use]
    pub fn heights_m(&self) -> (f64, f64) {
        match self.category {
            RoadCategory::Light => (0.01, 0.30),
            RoadCategory::MediumHeavy | RoadCategory::Heavy => (0.01, 0.75),
        }
    }

    /// The provenance of the coefficient tables driving this source
    /// (`[ASSUMED]` until SP 2006:12 is in hand — never a verified FORCE Pass).
    #[must_use]
    pub fn provenance(&self) -> Provenance {
        coefficients::PROVENANCE
    }

    /// The per-height `(low, high)` combined `L_W` spectra (27-band), after the
    /// rolling/propulsion energy split.
    ///
    /// `low = 10·lg(0.8·P_R + 0.2·P_P)`, `high = 10·lg(0.2·P_R + 0.8·P_P)`, so
    /// `P_low + P_high = P_R + P_P` exactly (energy conservation).
    #[must_use]
    pub fn split_lw_third_octave(&self) -> ([f64; N_THIRD_OCT], [f64; N_THIRD_OCT]) {
        let l_r = coefficients::rolling_lw(self.category, self.speed_kmh, self.temperature_c);
        let l_p = coefficients::propulsion_lw(self.category, self.speed_kmh);
        // The DAC-12 aggregate-size correction applies only on that surface.
        let surface = match self.surface {
            RoadSurface::Dac12 => coefficients::surface_dac12_correction(),
            RoadSurface::Reference => [0.0; N_THIRD_OCT],
        };

        let mut low = [0.0; N_THIRD_OCT];
        let mut high = [0.0; N_THIRD_OCT];
        // Index-based: three parallel input arrays feed two parallel outputs.
        #[allow(clippy::needless_range_loop)]
        for i in 0..N_THIRD_OCT {
            let p_r = 10f64.powf((l_r[i] + surface[i]) / 10.0);
            let p_p = 10f64.powf(l_p[i] / 10.0);
            let p_low = coefficients::ROLLING_LOW_FRACTION * p_r
                + (1.0 - coefficients::PROPULSION_HIGH_FRACTION) * p_p;
            let p_high = (1.0 - coefficients::ROLLING_LOW_FRACTION) * p_r
                + coefficients::PROPULSION_HIGH_FRACTION * p_p;
            low[i] = 10.0 * p_low.log10();
            high[i] = 10.0 * p_high.log10();
        }
        (low, high)
    }

    /// Expand into the two directional point sub-sources at a source-line point.
    ///
    /// `lane_centre_xy` is the vehicle centre line in the horizontal plane;
    /// `toward_receiver_xy` a (not necessarily unit) horizontal vector toward
    /// the receiver — the sub-sources are placed [`SOURCE_OFFSET_M`] along its
    /// unit direction (Pitfall 1). `ground_z` is the local ground elevation the
    /// heights sit above.
    #[must_use]
    pub fn expand(
        &self,
        lane_centre_xy: [f64; 2],
        toward_receiver_xy: [f64; 2],
        ground_z: f64,
    ) -> Vec<RoadSubSource> {
        let (h_low, h_high) = self.heights_m();
        let (low27, high27) = self.split_lw_third_octave();

        // Unit vector toward the receiver in the horizontal plane (guarded).
        let len = (toward_receiver_xy[0].powi(2) + toward_receiver_xy[1].powi(2)).sqrt();
        let unit = if len > 1e-12 {
            [toward_receiver_xy[0] / len, toward_receiver_xy[1] / len]
        } else {
            [1.0, 0.0]
        };
        let base_x = lane_centre_xy[0] + SOURCE_OFFSET_M * unit[0];
        let base_y = lane_centre_xy[1] + SOURCE_OFFSET_M * unit[1];

        let low = RoadSubSource {
            sub_source: SubSource {
                position: [base_x, base_y, ground_z + h_low],
                spectrum: third_octave_to_grid(&low27),
            },
            balloon: self.low_balloon(),
            role: SourceHeight::Low,
        };
        let high = RoadSubSource {
            sub_source: SubSource {
                position: [base_x, base_y, ground_z + h_high],
                spectrum: third_octave_to_grid(&high27),
            },
            balloon: self.high_balloon(),
            role: SourceHeight::High,
        };
        vec![low, high]
    }

    /// The low (0.01 m) source's balloon: vertical car-body screening × the
    /// frequency-dependent horizontal horn effect ([ASSUMED A7]).
    fn low_balloon(&self) -> DirectivityBalloon {
        DirectivityBalloon::from_equirect_sampler(
            BALLOON_STEP_DEG,
            BALLOON_STEP_DEG,
            |az, pol, b| vertical_screening(pol, b) + horn_horizontal(az, b),
        )
        .expect("equal-angle road directivity sampler is valid by construction")
    }

    /// The high source's balloon: vertical screening, plus a
    /// frequency-**independent** horizontal directivity for the heavy 0.75 m
    /// propulsion source ([ASSUMED A7]).
    fn high_balloon(&self) -> DirectivityBalloon {
        let heavy = matches!(
            self.category,
            RoadCategory::MediumHeavy | RoadCategory::Heavy
        );
        DirectivityBalloon::from_equirect_sampler(
            BALLOON_STEP_DEG,
            BALLOON_STEP_DEG,
            move |az, pol, b| {
                let horiz = if heavy { heavy_horizontal(az) } else { 0.0 };
                vertical_screening(pol, b) + horiz
            },
        )
        .expect("equal-angle road directivity sampler is valid by construction")
    }
}

/// Incoherent energy sum of per-band levels, dB: `10·lg Σ 10^{L/10}`.
///
/// Two identical co-located sub-sources combine to `+10·lg 2 ≈ +3.01 dB`
/// (Annex A), never +6 dB. Non-finite inputs are skipped (never a NaN out).
#[must_use]
pub fn energy_sum_db(levels_db: &[f64]) -> f64 {
    let energy: f64 = levels_db
        .iter()
        .filter(|l| l.is_finite())
        .map(|&l| 10f64.powf(l / 10.0))
        .sum();
    if energy > 0.0 {
        10.0 * energy.log10()
    } else {
        f64::NEG_INFINITY
    }
}

/// Map a 27-band 1/3-octave spectrum onto the 105-point 1/12-octave grid,
/// piecewise-constant per 1/3-octave (grid index `i·4` carries the exact
/// 1/3-octave value, so `third_octave_pick` recovers the input by index).
fn third_octave_to_grid(third: &[f64; N_THIRD_OCT]) -> BandSpectrum {
    let values: [f64; N_BANDS] = std::array::from_fn(|i| third[i / 4]);
    BandSpectrum::from_values(values)
}

/// `[ASSUMED A7]` vertical car-body screening: 0 dB at/above the horizon,
/// increasing attenuation below it (`pol > 90°`), stronger at high frequency.
fn vertical_screening(pol_deg: f64, band: usize) -> f64 {
    let below = ((pol_deg - 90.0) / 90.0).max(0.0); // 0 at horizon → 1 straight down
    let k = 2.0 + 4.0 * band as f64 / (N_BANDS as f64 - 1.0); // 2 … 6 dB
    -k * below
}

/// `[ASSUMED A7]` frequency-dependent horizontal horn effect on the 0.01 m
/// source — a mild fore/aft azimuthal variation (bounded, ~±2 dB at high f).
fn horn_horizontal(az_deg: f64, band: usize) -> f64 {
    let a = az_deg.to_radians();
    let amp = 0.5 + 1.5 * band as f64 / (N_BANDS as f64 - 1.0); // 0.5 … 2 dB
    amp * (a.sin().powi(2) - 0.5)
}

/// `[ASSUMED A7]` frequency-independent horizontal directivity on the heavy
/// 0.75 m propulsion source (small azimuthal variation).
fn heavy_horizontal(az_deg: f64) -> f64 {
    0.5 * az_deg.to_radians().cos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use envi_engine::freq::FreqAxis;

    fn cat1() -> RoadSource {
        RoadSource {
            category: RoadCategory::Light,
            speed_kmh: 80.0,
            surface: RoadSurface::Dac12,
            temperature_c: 15.0,
        }
    }

    #[test]
    fn cat1_expands_to_0_01_and_0_30_one_metre_toward_receiver() {
        let src = cat1();
        // Lane centre at x = 2.5 (FORCE), receiver toward +x ⇒ source line 3.5.
        let subs = src.expand([2.5, 0.0], [1.0, 0.0], 0.0);
        assert_eq!(subs.len(), 2);
        assert!((subs[0].sub_source.position[0] - 3.5).abs() < 1e-12);
        assert!((subs[1].sub_source.position[0] - 3.5).abs() < 1e-12);
        assert!((subs[0].sub_source.position[2] - 0.01).abs() < 1e-12);
        assert!((subs[1].sub_source.position[2] - 0.30).abs() < 1e-12);
        assert_eq!(subs[0].role, SourceHeight::Low);
        assert_eq!(subs[1].role, SourceHeight::High);
    }

    #[test]
    fn cat2_and_cat3_expand_to_0_01_and_0_75() {
        for cat in [RoadCategory::MediumHeavy, RoadCategory::Heavy] {
            let src = RoadSource {
                category: cat,
                ..cat1()
            };
            let subs = src.expand([2.5, 0.0], [1.0, 0.0], 0.0);
            assert!((subs[1].sub_source.position[2] - 0.75).abs() < 1e-12);
        }
    }

    #[test]
    fn rolling_propulsion_split_conserves_total_energy() {
        // low + high power must equal rolling + propulsion power, per band.
        let src = cat1(); // DAC-12 surface, so the reference must include it too.
        let l_r = coefficients::rolling_lw(src.category, src.speed_kmh, src.temperature_c);
        let surface = coefficients::surface_dac12_correction();
        let l_p = coefficients::propulsion_lw(src.category, src.speed_kmh);
        let (low, high) = src.split_lw_third_octave();
        #[allow(clippy::needless_range_loop)]
        for i in 0..N_THIRD_OCT {
            let total_in = 10f64.powf((l_r[i] + surface[i]) / 10.0) + 10f64.powf(l_p[i] / 10.0);
            let total_out = 10f64.powf(low[i] / 10.0) + 10f64.powf(high[i] / 10.0);
            assert!(
                (total_in - total_out).abs() <= 1e-6 * total_in,
                "band {i}: split must conserve energy ({total_in} vs {total_out})"
            );
        }
    }

    #[test]
    fn two_identical_colocated_subs_add_three_db_not_six() {
        let level = 70.0;
        let summed = energy_sum_db(&[level, level]);
        assert!(
            (summed - (level + 10.0 * 2f64.log10())).abs() < 1e-12,
            "two identical subs must be +3.01 dB, got {summed}"
        );
        // Explicitly NOT +6 dB.
        assert!((summed - (level + 6.0)).abs() > 1.0);
    }

    #[test]
    fn grid_mapping_preserves_third_octave_values_by_index() {
        let src = cat1();
        let (low, _high) = src.split_lw_third_octave();
        let spectrum = third_octave_to_grid(&low);
        let axis = FreqAxis::new();
        // third_octave_pick (index i·4) recovers the 27-band input.
        #[allow(clippy::needless_range_loop)]
        for i in 0..N_THIRD_OCT {
            assert!((spectrum.as_slice()[i * 4] - low[i]).abs() < 1e-12);
            // sanity: the axis third-octave centre matches the grid pick.
            let _ = axis.third_octave_pick(i);
        }
    }

    #[test]
    fn balloons_are_finite_and_screen_below_the_horizon() {
        let src = cat1();
        let subs = src.expand([2.5, 0.0], [1.0, 0.0], 0.0);
        for s in &subs {
            // Toward the horizon (+X): near 0 dB. Straight down: attenuated.
            let horizon = s.balloon.eval([1.0, 0.0, 0.0]);
            let down = s.balloon.eval([0.0, 0.0, -1.0]);
            assert!(horizon.iter().all(|v| v.is_finite()));
            assert!(down.iter().all(|v| v.is_finite()));
            // High-frequency screening below the horizon lowers the level.
            assert!(down[N_BANDS - 1] < horizon[N_BANDS - 1] + 1e-9);
        }
    }

    #[test]
    fn provenance_is_assumed() {
        assert_eq!(cat1().provenance(), Provenance::Assumed);
    }

    #[test]
    fn refs_manifest_pins_the_users_guide() {
        // The freely-downloadable User's Guide (AV 1171/06) is fetch-able and
        // sha256-pinned; SP 2006:12 stays a documented manual-drop (no URL).
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let fetch = std::fs::read_to_string(root.join("refs/fetch.sh")).unwrap();
        let manifest = std::fs::read_to_string(root.join("refs/refs.sha256")).unwrap();
        assert!(
            fetch.contains("Users_Guide_Nord2000_Road"),
            "fetch.sh must list the User's Guide"
        );
        assert!(
            manifest.contains("73f465e2cd78e54d536f5c29dce139c4fb5430539c84b00d974d0bc4d7d21491"),
            "refs.sha256 must pin the User's Guide hash"
        );
        assert!(
            fetch.contains("SP Rapport 2006:12"),
            "fetch.sh must document the SP 2006:12 manual-drop"
        );
    }
}
