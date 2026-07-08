"""Generate the committed refraction oracle fixture grid.

Writes ``crates/envi-harness/tests/fixtures/oracle/refraction.toml`` — a
cross-implementation reference the Rust engine's refraction primitives are
tested against. Regeneration is operator-driven (``python
gen_refraction_fixtures.py``); the TOML is committed and Python is NOT a build
dependency (Rust tests run against the committed TOML).

This oracle reimplements the AV 1106/07 rev.4 equations independently of the
engine (CalcEqSSP §5.5.2 Eqs. 15-21 + Annex F Eq. 403; DirectRay §5.5.4 Eqs.
29-44; ReflectedRay + cubic §5.5.5 Eqs. 45-50; TravelTimeDiff §5.5.6 Eqs.
51-53). It pins ξ, c₀ and Δτ. Per the oracle-independence caveat (D-04) it
cross-checks *implementation*, not the *spec reading* — the anchor + property
rungs guard the reading.

Equations are cited by AV 1106/07 report + number only — no document text or
figures are reproduced (licensing rule, CLAUDE.md).

Provenance: the fixture header records a sha256 of THIS generator (the eqssp /
circular-ray math lives here, not in common.py).
"""

from __future__ import annotations

import cmath
import hashlib
import math
from pathlib import Path

# Speed of sound at 15 C (AV 1106/07 Eq. 335: c = 20.05 * sqrt(t + 273.15)).
C0_M_S = 20.05 * math.sqrt(15.0 + 273.15)  # 340.348 m/s
Z0_MIN = 0.001
XI_HOM = 1e-6

# Soft-ground threshold: sigma < 1e7 Pa*s*m^-2 = 1e4 kPa*s*m^-2 (AV 1106/07
# Sec. 5.5.3). At or above it the ground is hard and CalcEqSSPGround is
# frequency-independent (delegates to CalcEqSSP).
SOFT_SIGMA_KPA_MAX = 1.0e4

# IEC 61260-1 base-10 octave ratio, matched bit-for-bit to the Rust constant
# envi_engine::freq::G so the 1/3-octave bracket in PhaseDiffFreq aligns.
G = 1.9952623149688795
N_THIRD_OCT = 27


def sound_speed_profile(z: float, a: float, b: float, c: float, z0: float) -> float:
    """Log-lin profile c(z) = A ln(z/z0 + 1) + B z + C (Eq. 2)."""
    z0 = max(z0, Z0_MIN)
    return a * math.log(z / z0 + 1.0) + b * z + c


def _antideriv_log(h: float, z0: float) -> float:
    """L(h) = (h+z0) ln(h/z0 + 1) - h (Annex F Eq. 403 log term)."""
    return (h + z0) * math.log(h / z0 + 1.0) - h


def _eq_ssp_terms(h_s: float, h_r: float, z0: float, a: float, b: float, c: float):
    """Geometry-dependent CalcEqSSP intermediates (dcdz, c_bar, hs_g, hr_g)."""
    z0 = max(z0, Z0_MIN)
    hmin = 5.0 * z0
    if abs(h_r - h_s) < 1e-9:
        hs_g = max(h_s - 0.005, hmin)
        hr_g = max(h_r + 0.005, hmin + 0.01)
    else:
        hs_g, hr_g = max(h_s, hmin), max(h_r, hmin)
    if abs(hr_g - hs_g) < 1e-12:
        hs_g, hr_g = hs_g - 0.005, hr_g + 0.005

    def c_of(z: float) -> float:
        return a * math.log(z / z0 + 1.0) + b * z + c

    dcdz = (c_of(hr_g) - c_of(hs_g)) / (hr_g - hs_g)  # Eq. 18
    c_bar = (
        a * (_antideriv_log(hr_g, z0) - _antideriv_log(hs_g, z0)) / (hr_g - hs_g)
        + b * (hr_g + hs_g) / 2.0
        + c
    )  # Eq. 19 / Annex F Eq. 403
    return dcdz, c_bar, hs_g, hr_g


def _collapse_gradient(dcdz: float, c_bar: float, hs_g: float, hr_g: float, c: float):
    """Collapse a (possibly frequency-modified) gradient to (xi, c0)."""
    c0 = c_bar - dcdz * (hs_g + hr_g) / 2.0  # Eq. 20
    xi = dcdz / c0  # Eq. 17
    if abs(xi) < XI_HOM:
        return 0.0, c
    return xi, c0


def calc_eq_ssp(h_s: float, h_r: float, z0: float, a: float, b: float, c: float):
    """CalcEqSSP -> (xi, c0) (Eqs. 15-21 + Annex F Eq. 403)."""
    dcdz, c_bar, hs_g, hr_g = _eq_ssp_terms(h_s, h_r, z0, a, b, c)
    return _collapse_gradient(dcdz, c_bar, hs_g, hr_g, c)


def _ground_impedance(f: float, sigma_kpa: float) -> complex:
    """Delany-Bazley Z_G (Eq. 57); sigma in kPa*s*m^-2."""
    x = f / sigma_kpa
    return complex(1.0 + 9.08 * x ** (-0.75), 11.9 * x ** (-0.73))


def _gamma_p(psi_g: float, z: complex) -> complex:
    """Plane-wave reflection coefficient Gamma_p (Eq. 59)."""
    s = math.sin(psi_g)
    inv_z = 1.0 / z
    return (s - inv_z) / (s + inv_z)


def _third_grid() -> list[float]:
    """The 27-point 1/3-octave grid (every 4th 1/12-oct centre), 25 Hz..10 kHz."""
    return [1000.0 * G ** ((4 * k - 64) / 12.0) for k in range(N_THIRD_OCT)]


def phase_diff_freq(
    d: float, h_s: float, h_r: float, sigma_min: float, c0: float, target_psi: float
) -> float:
    """PhaseDiffFreq (Eqs. 378-381) -> frequency at phase difference target_psi.

    An independent transcription of the Sub-model 2 auxiliary the engine reuses
    for CalcEqSSPGround's fL/fH (Eqs. 24-27).
    """
    r2 = math.sqrt(d * d + (h_s + h_r) ** 2)
    r1 = math.sqrt(d * d + (h_s - h_r) ** 2)
    dr = r2 - r1
    psi_g = math.asin((h_s + h_r) / r2)
    tau = 2.0 * math.pi

    def psi_of(f: float) -> float:
        try:
            arg_gp = cmath.phase(_gamma_p(psi_g, _ground_impedance(f, sigma_min)))
        except (ValueError, ZeroDivisionError):
            arg_gp = 0.0
        return tau * f * dr / c0 + arg_gp

    grid = _third_grid()
    f_lo, f_hi = grid[0], grid[-1]
    psi_lo, psi_hi = psi_of(f_lo), psi_of(f_hi)
    if target_psi <= psi_lo:
        if abs(psi_lo) < 1e-30:
            return f_lo
        return max(f_lo * target_psi / psi_lo, 1e-6)
    if target_psi >= psi_hi:
        f_8k = grid[N_THIRD_OCT - 3]
        psi_8k = psi_of(f_8k)
        slope = (f_hi - f_8k) / max(psi_hi - psi_8k, 1e-30)
        f = f_hi + (target_psi - psi_hi) * slope
        return min(max(f, f_hi), 100_000.0)
    for i in range(len(grid) - 1):
        f1, f2 = grid[i], grid[i + 1]
        p1, p2 = psi_of(f1), psi_of(f2)
        if p1 <= target_psi <= p2:
            if abs(p2 - p1) < 1e-30:
                return f1
            log_f = math.log(f1) + (target_psi - p1) / (p2 - p1) * (
                math.log(f2) - math.log(f1)
            )
            return math.exp(log_f)
    return f_hi


def calc_eq_ssp_ground(
    f: float,
    d: float,
    h_s: float,
    h_r: float,
    sigma_kpa: float,
    z0: float,
    a: float,
    b: float,
    c: float,
):
    """CalcEqSSPGround -> (xi(f), c0(f)) (Eqs. 22-28)."""
    if sigma_kpa >= SOFT_SIGMA_KPA_MAX:  # hard ground -> frequency-independent
        return calc_eq_ssp(h_s, h_r, z0, a, b, c)

    dcdz, c_bar, hs_g, hr_g = _eq_ssp_terms(h_s, h_r, z0, a, b, c)

    d_c = min(d, 400.0)  # Eq. 24/25 clamps
    hs_c = max(h_s, 0.5)
    hr_c = max(h_r, 0.5)
    f_psi = phase_diff_freq(d_c, hs_c, hr_c, sigma_kpa, c, math.pi)
    f_2psi = phase_diff_freq(d_c, hs_c, hr_c, sigma_kpa, c, 2.0 * math.pi)

    dc10 = sound_speed_profile(10.0, a, b, c, z0) - sound_speed_profile(0.0, a, b, c, z0)
    if dc10 <= 1.0:  # Eq. 26 (piecewise, C0-continuous at 1 and 5)
        factor = 1.0
    elif dc10 >= 5.0:
        factor = 0.7
    else:
        factor = (43.0 - 3.0 * dc10) / 40.0
    f_l = factor * f_psi
    f_h = max(f_2psi, 1.25 * f_l)  # Eq. 27

    if f >= f_h:  # Eq. 23
        dcdz_eff = dcdz
    elif f <= f_l:
        dcdz_eff = 0.0
    else:
        k = (math.log(f) - math.log(f_l)) / (math.log(f_h) - math.log(f_l))
        dcdz_eff = k * dcdz
    return _collapse_gradient(dcdz_eff, c_bar, hs_g, hr_g, c)


def _direct_ray(d: float, h_a: float, h_b: float, xi: float, c0: float):
    """DirectRay -> (tau, R, d_sz) (Eqs. 29-44)."""
    z_l, z_u = (h_a, h_b) if h_a <= h_b else (h_b, h_a)
    dz = z_u - z_l
    if dz < 0.01:
        dz = 0.01
    denom = 1.0 + xi * z_l
    c0_l = c0 * denom  # Eq. 31
    xi_l = xi / denom  # Eq. 30
    if abs(xi_l) < 1e-10:
        xi_l = -1e-10 if xi_l < 0.0 else 1e-10
    xa = abs(xi_l)
    tan_psi = xa * d / 2.0 + dz * (2.0 + xa * dz) / (2.0 * d)  # Eq. 32
    psi = math.atan(tan_psi)
    dm = tan_psi / xa  # Eq. 33
    cp = math.cos(psi)

    def r_of(dzv: float) -> float:  # Eq. 34
        return (math.asin((1.0 + xa * dzv) * cp) - math.pi / 2.0 + psi) / (xa * cp)

    def tau_of(dzv: float) -> float:  # Eqs. 35-37
        f0 = (1.0 + math.sin(psi)) / (1.0 - math.sin(psi))
        s = math.sqrt(max(0.0, 1.0 - (1.0 + xa * dzv) ** 2 * cp * cp))
        fz = (1.0 + s) / (1.0 - s)
        return math.log(f0 / fz) / (2.0 * xa * c0_l)

    if d <= dm:
        r, tau = r_of(dz), tau_of(dz)
    else:
        dz_m = (1.0 / xa) * (1.0 / cp - 1.0)  # Eq. 38
        r = 2.0 * r_of(dz_m) - r_of(dz)  # Eq. 39
        tau = 2.0 * tau_of(dz_m) - tau_of(dz)  # Eq. 40
    if xi >= 0.0:
        d_sz = math.inf
    else:
        rl = z_l * (2.0 / xa - z_l)
        ru = z_u * (2.0 / xa - z_u)
        d_sz = math.sqrt(max(0.0, rl)) + math.sqrt(max(0.0, ru))  # Eq. 43
    return tau, r, d_sz, psi


def _cubic_reflection(d: float, h_s: float, h_r: float, xi: float) -> float:
    """Reflection-point d_refl (Eq. 49): 2x^3 - 3d x^2 + (bR2+bS2+d2)x - bS2 d."""
    b_s2 = (h_s / xi) * (2.0 + xi * h_s)
    b_r2 = (h_r / xi) * (2.0 + xi * h_r)
    # Monic: x^3 + p2 x^2 + p1 x + p0.
    p2 = -3.0 * d / 2.0
    p1 = (b_r2 + b_s2 + d * d) / 2.0
    p0 = -b_s2 * d / 2.0
    roots = _real_cubic_roots(p2, p1, p0)
    cand = [r for r in roots if 1e-9 < r < d - 1e-9]
    if not cand:
        raise ValueError("no reflection root in (0,d)")
    return min(cand) if h_s < h_r else max(cand)


def _real_cubic_roots(a2: float, a1: float, a0: float):
    """Real roots of x^3 + a2 x^2 + a1 x + a0 (depressed-cubic trig/Cardano)."""
    p = a1 - a2 * a2 / 3.0
    q = 2.0 * a2 ** 3 / 27.0 - a2 * a1 / 3.0 + a0
    shift = -a2 / 3.0
    disc = (q / 2.0) ** 2 + (p / 3.0) ** 3
    if disc <= 0.0 and p < 0.0:
        m = 2.0 * math.sqrt(-p / 3.0)
        arg = max(-1.0, min(1.0, 3.0 * q / (p * m)))
        theta = math.acos(arg) / 3.0
        return [
            m * math.cos(theta) + shift,
            m * math.cos(theta - 2.0 * math.pi / 3.0) + shift,
            m * math.cos(theta - 4.0 * math.pi / 3.0) + shift,
        ]
    s = math.sqrt(disc)
    u = math.copysign(abs(-q / 2.0 + s) ** (1.0 / 3.0), -q / 2.0 + s)
    v = math.copysign(abs(-q / 2.0 - s) ** (1.0 / 3.0), -q / 2.0 - s)
    return [u + v + shift]


def travel_time_diff(d: float, h_s: float, h_r: float, xi: float, c0: float) -> float:
    """Delta-tau = tau2 - tau1 with the Eq. 52 shadow-edge cap."""
    tau1, _, d_sz, _ = _direct_ray(d, h_s, h_r, xi, c0)
    d_refl = _cubic_reflection(d, h_s, h_r, xi)
    tau_s, _, _, _ = _direct_ray(d_refl, 0.0, h_s, xi, c0)
    tau_r, _, _, _ = _direct_ray(d - d_refl, 0.0, h_r, xi, c0)
    dtau = (tau_s + tau_r) - tau1  # Eqs. 45-46, 51
    if xi < 0.0:
        if d > 0.95 * d_sz:
            return 0.0
        dtau0 = (
            (1.0 - (d / d_sz) ** 2)
            * (
                math.sqrt(d * d + (h_s + h_r) ** 2)
                - math.sqrt(d * d + (h_s - h_r) ** 2)
            )
            / c0
        )  # Eq. 52
        if dtau > dtau0:
            dtau = dtau0
    return dtau


# CalcEqSSP grid: (A, B, C, z0, hS, hR) covering up/down gradients + equal
# heights + varied roughness.
EQSSP_GRID = [
    (0.5, 0.05, C0_M_S, 0.001, 0.5, 1.5),
    (1.0, 0.10, C0_M_S, 0.01, 0.5, 5.0),
    (-0.8, -0.04, C0_M_S, 0.05, 1.0, 4.0),
    (2.0, 0.02, C0_M_S, 0.1, 0.5, 10.0),
    (0.3, 0.00, C0_M_S, 0.02, 2.0, 2.0),
    (-1.5, 0.06, C0_M_S, 0.001, 0.5, 20.0),
    (1.2, -0.03, C0_M_S, 0.03, 3.0, 8.0),
    (0.9, 0.08, C0_M_S, 0.005, 0.5, 1.5),
]

# CalcEqSSPGround grid: soft- and hard-ground configs pinned at representative
# BAND INDICES (D-14). Each tuple is (A, B, C, z0, hS, hR, d, sigma_kpa); the
# fixture stores xi(f)/c0(f) at the band indices in GROUND_BAND_IDX below.
EQSSP_GROUND_GRID = [
    # Soft grassland, downward refraction (inversion) — the MET-04 target class.
    (0.6, 0.05, C0_M_S, 0.02, 0.5, 1.5, 97.5, 200.0),
    # Soft ground, upward refraction (lapse) — negative gradient.
    (-0.8, -0.04, C0_M_S, 0.05, 0.5, 2.0, 150.0, 500.0),
    # Soft ground, homogeneous (A=B=0) — must be (0, C) at every band.
    (0.0, 0.0, C0_M_S, 0.001, 0.5, 1.5, 97.5, 200.0),
    # Hard asphalt-class ground — frequency-independent (flat ξ across bands).
    (0.6, 0.05, C0_M_S, 0.001, 0.5, 1.5, 97.5, 20000.0),
]

# Band indices sampled across the 105-point grid (low<fL, mid-interpolated,
# high). f(idx) = 1000 * G**((idx-64)/12).
GROUND_BAND_IDX = [0, 16, 32, 48, 64, 80, 96, 104]


def _band_freq(idx: int) -> float:
    return 1000.0 * G ** ((idx - 64) / 12.0)


# Delta-tau grid: downward (xi>0) and a mild upward (xi<0) geometry, several
# horizontal distances (interference is Delta-tau-driven).
DTAU_GRID = [
    (0.6, 0.05, 0.01, 0.5, 1.5, 50.0),
    (0.6, 0.05, 0.01, 0.5, 1.5, 100.0),
    (1.0, 0.10, 0.01, 0.5, 3.0, 200.0),
    (1.5, 0.08, 0.001, 0.5, 1.5, 300.0),
    (2.0, 0.02, 0.05, 1.0, 4.0, 500.0),
    (-0.5, -0.02, 0.01, 0.5, 1.5, 100.0),
    (-0.4, -0.01, 0.01, 1.0, 2.0, 150.0),
]


def _fmt(x: float) -> str:
    return repr(float(x))


def main() -> None:
    here = Path(__file__).resolve().parent
    root = here.parents[1]
    out = root / "crates/envi-harness/tests/fixtures/oracle/refraction.toml"
    out.parent.mkdir(parents=True, exist_ok=True)

    sha = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()[:16]

    lines: list[str] = []
    lines.append(
        "# generated by tools/nord2000_oracle/gen_refraction_fixtures.py — DO NOT EDIT"
    )
    lines.append(
        "# Cross-implementation oracle: AV 1106/07 Eqs. 15-21, 29-53, Annex F 403."
    )
    lines.append("")
    lines.append("[meta]")
    lines.append('oracle = "AV 1106/07 CalcEqSSP + DirectRay/ReflectedRay/TravelTimeDiff"')
    lines.append(f'provenance = "gen_refraction_fixtures.py sha256:{sha}"')
    lines.append(f"c0_m_s = {_fmt(C0_M_S)}")
    lines.append("eqssp_tol_rel = 1e-9")
    # Δτ is a difference of near-equal circular travel times (AV §5.5.6 warns
    # explicitly); its cross-implementation precision is cancellation-limited to
    # ~1e-5 relative. 1e-4 is the Phase-2 oracle-style gate — still tight enough
    # that any mistranscribed exponent/sign in Eqs. 32–52 fails by >1%.
    lines.append("dtau_tol_rel = 1e-4")
    # CalcEqSSPGround xi(f)/c0(f) depend on PhaseDiffFreq (transcendental ln/asin/
    # atan2 across a 1/3-oct bracket); Rust vs Python last-ULP drift makes 1e-9
    # too tight. 1e-6 relative still fails any mistranscribed Eq. 23/26/27 by >0.01%.
    lines.append("eqssp_ground_tol_rel = 1e-6")
    lines.append("")

    for a, b, c, z0, h_s, h_r, d, sigma_kpa in EQSSP_GROUND_GRID:
        for idx in GROUND_BAND_IDX:
            f = _band_freq(idx)
            xi, c0 = calc_eq_ssp_ground(f, d, h_s, h_r, sigma_kpa, z0, a, b, c)
            lines.append("[[eqssp_ground]]")
            for k, v in (
                ("a", a), ("b", b), ("c", c), ("z0", z0),
                ("h_s", h_s), ("h_r", h_r), ("d", d), ("sigma_kpa", sigma_kpa),
            ):
                lines.append(f"{k} = {_fmt(v)}")
            lines.append(f"band_index = {idx}")
            lines.append(f"xi = {_fmt(xi)}")
            lines.append(f"c0 = {_fmt(c0)}")
            lines.append("")

    for a, b, c, z0, h_s, h_r in EQSSP_GRID:
        xi, c0 = calc_eq_ssp(h_s, h_r, z0, a, b, c)
        lines.append("[[eqssp]]")
        for k, v in (("a", a), ("b", b), ("c", c), ("z0", z0), ("h_s", h_s), ("h_r", h_r)):
            lines.append(f"{k} = {_fmt(v)}")
        lines.append(f"xi = {_fmt(xi)}")
        lines.append(f"c0 = {_fmt(c0)}")
        lines.append("")

    for a, b, z0, h_s, h_r, d in DTAU_GRID:
        c = C0_M_S
        xi, c0 = calc_eq_ssp(h_s, h_r, z0, a, b, c)
        dtau = travel_time_diff(d, h_s, h_r, xi, c0)
        lines.append("[[dtau]]")
        for k, v in (
            ("a", a), ("b", b), ("c", c), ("z0", z0),
            ("h_s", h_s), ("h_r", h_r), ("d", d),
        ):
            lines.append(f"{k} = {_fmt(v)}")
        lines.append(f"xi = {_fmt(xi)}")
        lines.append(f"c0 = {_fmt(c0)}")
        lines.append(f"dtau = {_fmt(dtau)}")
        lines.append("")

    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    n_ground = len(EQSSP_GROUND_GRID) * len(GROUND_BAND_IDX)
    print(
        f"wrote {out} ({len(EQSSP_GRID)} eqssp + {n_ground} eqssp_ground "
        f"+ {len(DTAU_GRID)} dtau rows)"
    )


if __name__ == "__main__":
    main()
