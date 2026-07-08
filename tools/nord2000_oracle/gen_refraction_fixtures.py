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

import hashlib
import math
from pathlib import Path

# Speed of sound at 15 C (AV 1106/07 Eq. 335: c = 20.05 * sqrt(t + 273.15)).
C0_M_S = 20.05 * math.sqrt(15.0 + 273.15)  # 340.348 m/s
Z0_MIN = 0.001
XI_HOM = 1e-6


def sound_speed_profile(z: float, a: float, b: float, c: float, z0: float) -> float:
    """Log-lin profile c(z) = A ln(z/z0 + 1) + B z + C (Eq. 2)."""
    z0 = max(z0, Z0_MIN)
    return a * math.log(z / z0 + 1.0) + b * z + c


def _antideriv_log(h: float, z0: float) -> float:
    """L(h) = (h+z0) ln(h/z0 + 1) - h (Annex F Eq. 403 log term)."""
    return (h + z0) * math.log(h / z0 + 1.0) - h


def calc_eq_ssp(h_s: float, h_r: float, z0: float, a: float, b: float, c: float):
    """CalcEqSSP -> (xi, c0) (Eqs. 15-21 + Annex F Eq. 403)."""
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
    c0 = c_bar - dcdz * (hs_g + hr_g) / 2.0  # Eq. 20
    xi = dcdz / c0  # Eq. 17
    if abs(xi) < XI_HOM:
        return 0.0, c
    return xi, c0


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
    print(f"wrote {out} ({len(EQSSP_GRID)} eqssp + {len(DTAU_GRID)} dtau rows)")


if __name__ == "__main__":
    main()
