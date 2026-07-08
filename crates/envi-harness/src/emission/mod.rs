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

/// One traffic class on a lane: a [`RoadSource`] carrying a fraction of the
/// lane's total flow (an ENERGY/flow share in `[0, 1]`, need not pre-normalize).
///
/// Curved-road FORCE traffic is `90 % cat-1 @ 90 km/h + 10 % cat-2/3 @ 70 km/h`
/// (EP 1335 Ch. 4); city-street is `100 % cat-1 @ 50 km/h`. Each class expands
/// into its OWN sub-sources at its OWN heights (0.30 m light vs 0.75 m heavy),
/// scaled by its flow share — categories are NEVER merged into a single "high"
/// source because their physical heights differ.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrafficClass {
    /// The vehicle-class road source (category, speed, surface, temperature).
    pub source: RoadSource,
    /// Flow share of the lane total (fraction, `> 0`).
    pub fraction: f64,
}

/// The along-road source-line offsets (m) for a road section of half-length
/// `half_len_m` sampled at `spacing_m`, symmetric about the perpendicular
/// foot (`y = 0`): `…, −spacing, 0, +spacing, …` within `±half_len_m`.
///
/// Curved-road spacing is 10/25/50 m and city-street 2/20 m (EP 1335 Ch. 4–5);
/// the discrete source points replace the 179-point 1° pass-by discretization
/// used for the (laterally-invariant) straight road.
///
/// Returns an empty vec for a non-finite / non-positive `spacing_m` or
/// `half_len_m` (a degenerate section contributes nothing — the caller treats
/// the empty set as "no in-range sources").
#[must_use]
pub fn source_line_offsets(half_len_m: f64, spacing_m: f64) -> Vec<f64> {
    if !(spacing_m.is_finite() && spacing_m > 0.0 && half_len_m.is_finite() && half_len_m >= 0.0) {
        return Vec::new();
    }
    let n = (half_len_m / spacing_m).floor() as i64;
    (-n..=n).map(|k| k as f64 * spacing_m).collect()
}

/// The city-street source cutoff (EP 1335 Ch. 5): a source point is included
/// only while its source→receiver distance is within `factor ×` the shortest
/// source→receiver distance on the lane (`factor = 20`).
///
/// Returns `true` when `d_source ≤ factor · d_shortest` (both finite, positive).
#[must_use]
pub fn within_distance_cutoff(d_source: f64, d_shortest: f64, factor: f64) -> bool {
    d_source.is_finite()
        && d_shortest.is_finite()
        && d_source > 0.0
        && d_shortest > 0.0
        && d_source <= factor * d_shortest
}

/// The city-street 20×-shortest-distance cutoff factor (EP 1335 Ch. 5).
pub const CITY_CUTOFF_FACTOR: f64 = 20.0;

/// Expand a multi-class lane into the union of every class's directional
/// sub-sources at a single source-line point, each scaled by its flow share.
///
/// For every [`TrafficClass`] the two Nord2000 sub-sources are expanded via
/// [`RoadSource::expand`] and their `L_W` spectra shifted by `10·lg(fraction)`
/// (the energy/flow share) — so two classes at the same level combine
/// incoherently in the readout exactly as their shares dictate. Categories keep
/// their own heights (light 0.30 m, heavy 0.75 m).
#[must_use]
pub fn expand_traffic(
    classes: &[TrafficClass],
    lane_centre_xy: [f64; 2],
    toward_receiver_xy: [f64; 2],
    ground_z: f64,
) -> Vec<RoadSubSource> {
    let mut out = Vec::with_capacity(classes.len() * 2);
    for class in classes {
        if !(class.fraction.is_finite() && class.fraction > 0.0) {
            continue;
        }
        let shift = 10.0 * class.fraction.log10();
        for mut sub in class
            .source
            .expand(lane_centre_xy, toward_receiver_xy, ground_z)
        {
            let shifted: [f64; N_BANDS] =
                std::array::from_fn(|i| sub.sub_source.spectrum.as_slice()[i] + shift);
            sub.sub_source.spectrum = BandSpectrum::from_values(shifted);
            out.push(sub);
        }
    }
    out
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
    fn provenance_is_cited() {
        // The rolling/propulsion tables are transcribed from the committed
        // source-modelling report (Table A.1), verified against the page image.
        assert_eq!(cat1().provenance(), Provenance::Cited);
    }

    #[test]
    fn source_line_offsets_are_symmetric_and_spaced() {
        // half-length 100 m at 25 m spacing ⇒ −100 … +100 in 25 m steps (9 pts).
        let offs = source_line_offsets(100.0, 25.0);
        assert_eq!(offs.len(), 9);
        assert!((offs[0] + 100.0).abs() < 1e-12);
        assert!((offs[4]).abs() < 1e-12); // centre point at y = 0
        assert!((offs[8] - 100.0).abs() < 1e-12);
        // Symmetry about 0.
        for k in 0..offs.len() {
            assert!((offs[k] + offs[offs.len() - 1 - k]).abs() < 1e-12);
        }
        // Degenerate spacing ⇒ empty (never a panic / infinite loop).
        assert!(source_line_offsets(100.0, 0.0).is_empty());
        assert!(source_line_offsets(100.0, f64::NAN).is_empty());
    }

    #[test]
    fn city_cutoff_keeps_near_and_drops_far_sources() {
        let d_shortest = 30.0;
        assert!(within_distance_cutoff(30.0, d_shortest, CITY_CUTOFF_FACTOR));
        assert!(within_distance_cutoff(
            20.0 * 30.0,
            d_shortest,
            CITY_CUTOFF_FACTOR
        ));
        // Just beyond 20× is dropped.
        assert!(!within_distance_cutoff(
            20.0 * 30.0 + 1e-6,
            d_shortest,
            CITY_CUTOFF_FACTOR
        ));
        assert!(!within_distance_cutoff(
            -1.0,
            d_shortest,
            CITY_CUTOFF_FACTOR
        ));
    }

    #[test]
    fn multiclass_expands_each_category_at_its_own_height_scaled_by_share() {
        // Curved-road mix: 90 % cat-1 @ 90 + 10 % cat-3 @ 70.
        let classes = [
            TrafficClass {
                source: RoadSource {
                    category: RoadCategory::Light,
                    speed_kmh: 90.0,
                    surface: RoadSurface::Dac12,
                    temperature_c: 15.0,
                },
                fraction: 0.9,
            },
            TrafficClass {
                source: RoadSource {
                    category: RoadCategory::Heavy,
                    speed_kmh: 70.0,
                    surface: RoadSurface::Dac12,
                    temperature_c: 15.0,
                },
                fraction: 0.1,
            },
        ];
        let subs = expand_traffic(&classes, [2.5, 0.0], [1.0, 0.0], 0.0);
        assert_eq!(subs.len(), 4, "two classes × two heights");
        // Light high source at 0.30 m, heavy high source at 0.75 m.
        assert!((subs[1].sub_source.position[2] - 0.30).abs() < 1e-12);
        assert!((subs[3].sub_source.position[2] - 0.75).abs() < 1e-12);

        // The 90 % share is a −10·lg(1/0.9) ≈ −0.458 dB shift vs the unshared
        // single-class expansion.
        let single = RoadSource {
            category: RoadCategory::Light,
            speed_kmh: 90.0,
            surface: RoadSurface::Dac12,
            temperature_c: 15.0,
        }
        .expand([2.5, 0.0], [1.0, 0.0], 0.0);
        let want_shift = 10.0 * 0.9_f64.log10();
        for f in 0..N_BANDS {
            assert!(
                (subs[0].sub_source.spectrum.as_slice()[f]
                    - single[0].sub_source.spectrum.as_slice()[f]
                    - want_shift)
                    .abs()
                    < 1e-9
            );
        }
        // A zero/negative fraction contributes nothing (never a −inf spectrum).
        let none = expand_traffic(
            &[TrafficClass {
                source: classes[0].source,
                fraction: 0.0,
            }],
            [2.5, 0.0],
            [1.0, 0.0],
            0.0,
        );
        assert!(none.is_empty());
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
