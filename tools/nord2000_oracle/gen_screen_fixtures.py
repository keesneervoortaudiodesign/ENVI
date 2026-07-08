"""Independent Nord2000 screen-over-ground oracle (Sub-model 4 + Sub-model 7).

Cross-implementation reference for the ENVI Rust engine's
``propagation::terrain_effect::{screen, submodel7}`` modules. Implements the
single-screen four-path image model (AV 1106/07 §5.13, Eqs. 157-188) and the
turbulence-scattering floor (§5.16, Eqs. 271-274) independently, and pins the
combined ``ΔL₄ + ΔL₇`` curve for the literal case-71 thin-screen geometry as a
committed TOML fixture (``crates/envi-harness/tests/fixtures/oracle/screen_thin.toml``).

Independence: the wedge diffraction ``pwedge`` and the wedge-/ground-face
spherical-wave coefficient ``Q̂`` are imported from the sibling wedge oracle and
``common.py`` (which obtains the Faddeeva ``w(z)`` from ``scipy.special.wofz`` --
a mature, independent implementation). If the Rust engine's four-path
combination, its ``w(z)``, or its Table 6/7 transcription has an error, this
oracle disagrees.

Both implementations use the SAME Eq. 187-188 reading (page images pp. 79-80) and
the SAME Table 6/7 transcription (pp. 117-118). Wide flat reflecting strips are
used on each side so the Fresnel-zone weight saturates to exactly 1 (w_Q = 1,
w′ = 1) in both -- the base-model single-combination case -- so the cross-check
targets the combination + scattering math, not the Fresnel-weight machinery
(covered by the engine's own unit tests).

Equations cited by AV 1106/07 report + number only (CLAUDE.md licensing). Developer
tool emitting committed fixture DATA; NOT a build dependency.

Nord2000-native complex convention: time e^{-jwt}, outgoing phase e^{+jwt}.
"""

from __future__ import annotations

import cmath
import hashlib
import math
from pathlib import Path

from common import C0_M_S, ground_impedance, spherical_q
from gen_wedge_fixtures import HARD, pwedge

# --- Sub-model 7 Tables 6/7 (AV 1106/07 pp. 117-118, transcribed from images) --
COL_AXIS = [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0]
ROW_AXIS = [5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0]
TABLE6 = [
    [-41.6, -33.9, -30.2, -27.9, -26.2, -24.9, -23.9, -23.1, -22.4, -21.8],
    [-49.9, -44.0, -39.5, -36.7, -34.7, -33.1, -31.8, -30.9, -30.1, -29.3],
    [-52.1, -48.9, -45.8, -42.9, -40.7, -39.1, -37.7, -36.6, -35.5, -34.7],
    [-53.8, -51.0, -48.8, -46.8, -45.0, -43.5, -42.1, -40.9, -39.9, -39.1],
    [-55.4, -52.4, -50.4, -48.8, -47.5, -46.2, -45.1, -44.0, -43.1, -42.2],
    [-57.0, -53.8, -51.5, -50.6, -48.9, -47.8, -46.8, -45.9, -45.2, -44.4],
    [-58.6, -55.1, -52.7, -51.1, -49.8, -48.8, -47.9, -47.1, -46.4, -45.7],
    [-59.9, -56.5, -53.9, -52.1, -50.7, -49.6, -48.7, -48.0, -47.3, -46.6],
]
TABLE7 = [
    [-44.0, -39.1, -36.0, -34.0, -32.5, -31.3, -30.4, -29.6, -28.9, -28.3],
    [-47.4, -44.7, -42.4, -40.5, -39.1, -37.9, -36.9, -36.0, -35.3, -34.7],
    [-48.9, -46.7, -45.1, -43.6, -42.4, -41.4, -40.5, -39.7, -39.0, -38.4],
    [-50.2, -48.0, -46.4, -45.2, -44.1, -43.2, -42.4, -41.7, -41.1, -40.5],
    [-51.4, -49.0, -47.4, -46.2, -45.2, -44.3, -43.6, -42.9, -42.3, -41.8],
    [-52.5, -50.0, -48.3, -47.0, -46.0, -45.1, -44.4, -43.7, -43.2, -42.6],
    [-53.6, -51.0, -49.2, -47.8, -46.7, -45.8, -45.0, -44.4, -43.8, -43.3],
    [-54.6, -52.0, -50.0, -48.5, -47.4, -46.4, -45.6, -44.9, -44.3, -43.8],
]
NO_SCATTER_DB = -300.0


def _dist(a, b):
    return math.hypot(b[0] - a[0], b[1] - a[1])


def _wedge_angles(w1, t, w2, s, r):
    """Wedge angles (β, θ_S, θ_R) from screen coordinates (Eq. 162)."""
    b1 = math.atan((w1[1] - t[1]) / (t[0] - w1[0])) + math.pi / 2.0
    b2 = math.atan((w2[1] - t[1]) / (w2[0] - t[0])) + math.pi / 2.0
    t1 = math.atan((s[1] - t[1]) / (t[0] - s[0])) + math.pi / 2.0
    t2 = math.atan((r[1] - t[1]) / (r[0] - t[0])) + math.pi / 2.0
    return (2 * math.pi - b1 - b2, 2 * math.pi - t1 - b2, t2 - b2)


def _pwedge_pt(f, w1, t, w2, s, r):
    """Diffracted pressure over the screen for source s, receiver r."""
    beta, ts, tr = _wedge_angles(w1, t, w2, s, r)
    r_s = _dist(s, t)
    r_r = _dist(t, r)
    ell = r_s + r_r
    return pwedge(f, beta, ts, tr, ell / C0_M_S, r_s / C0_M_S, r_r / C0_M_S,
                  ell, r_s, r_r, HARD, HARD)


def _reflect_flat(ep, top, sigma, f):
    """Flat-ground (z=0) reflection of endpoint ep towards the screen top.

    Returns (image, Q̂, dtau, rho, r1_leg, r2_leg). Mirrors the engine's
    straight_rays_over_segment + spherical_q on a flat z=0 segment.
    """
    x0, z0 = ep
    xt, zt = top
    image = (x0, -z0)
    # Reflection point where image->top crosses z=0.
    xr = x0 + z0 * (xt - x0) / (zt + z0)
    r1_leg = math.hypot(xr - x0, z0)
    r2_leg = math.hypot(xt - xr, zt)
    tau2 = (r1_leg + r2_leg) / C0_M_S
    psi_g = math.atan2(z0, xr - x0)
    zg = ground_impedance(f, sigma)
    q = spherical_q(f, tau2, psi_g, zg)
    # Cancellation-free ΔR = 4 z0 zt / (R2 + R1).
    r1 = _dist(ep, top)
    r2 = _dist(image, top)
    dtau = (4.0 * z0 * zt / (r2 + r1)) / C0_M_S
    h1 = r1_leg * math.sin(psi_g)
    h2 = r2_leg * math.sin(psi_g)
    rho = 2.0 * h1 * h2 / (h1 + h2) if h1 + h2 > 0 else 0.0
    return image, q, dtau, rho, tau2, zg


def _rho_i(zg):
    """Incoherent reflection coefficient ρᵢ = √(1 − ᾱ_ri) (Eqs. 75-76)."""
    x, y = zg.real, zg.imag
    m = x * x + y * y
    alpha = (8.0 * x / m) * (
        1.0 - (x / m) * math.log((1.0 + x) ** 2 + y * y)
        + ((x * x - y * y) / (y * m)) * math.atan2(y, 1.0 + x)
    )
    return math.sqrt(max(0.0, min(1.0, 1.0 - alpha)))


def _calc_fz_d(r_s, r_r, theta, flp):
    """CalcFZd (AV 1106/07 Eq. 339) — Fresnel-ellipse reach in direction theta."""
    r = r_s + r_r
    ell = r + flp
    ct = math.cos(theta)
    a = 4.0 * (ell * ell - (r * ct) ** 2)
    b = 4.0 * r * ct * (r_r * r_r - r_s * r_s) + 4.0 * (r_s - r_r) * ell * ell * ct
    c = -(ell ** 4) + 2.0 * (r_s * r_s + r_r * r_r) * ell * ell - (r_s * r_s - r_r * r_r) ** 2
    disc = b * b - 4.0 * a * c
    return (-b + math.sqrt(disc)) / (2.0 * a)


def _fresnel_zone_size(d, h_s, h_r, flp):
    """FresnelZoneSize (Eqs. 340-344)."""
    hsum = h_s + h_r
    psi = math.atan(hsum / d)
    r = math.hypot(hsum, d)
    r_s = h_s / hsum * r
    r_r = h_r / hsum * r
    a1 = _calc_fz_d(r_s, r_r, math.pi - psi, flp)
    a2 = _calc_fz_d(r_s, r_r, psi, flp)
    bp = _calc_fz_d(r_s, r_r, math.pi / 2.0, flp)
    denom = 1.0 - ((a2 - a1) / (a2 + a1)) ** 2
    b = math.sqrt(bp * bp / denom) if denom > 0.0 else bp
    return a1, a2, b


def _zone_borders(d, h_s, h_r, flp):
    h_s = max(h_s, 0.01)
    h_r = max(h_r, 0.01)
    d_refl = d * h_s / (h_s + h_r)
    a1, a2, _ = _fresnel_zone_size(d, h_s, h_r, flp)
    return d_refl, d_refl - a1, d_refl + a2


def _overlap(lo, hi, a, b):
    width = b - a
    if width <= 0.0:
        return 0.0
    left = max(lo, a)
    right = min(hi, b)
    return max(0.0, min(1.0, max(0.0, right - left) / width))


def _fresnel_zone_w(d, h_s, h_r, d1, d2, flp):
    _, d1fz, d2fz = _zone_borders(d, h_s, h_r, flp)
    return _overlap(d1, d2, d1fz, d2fz)


def _segment_variables(a, b, seg_a, seg_b):
    """SegmentVariables (Eq. 383) — mirrors the engine's norm_line frame."""
    e = (seg_b[0] - seg_a[0], seg_b[1] - seg_a[1])
    length = math.hypot(*e)
    ehat = (e[0] / length, e[1] / length)
    nhat = (-ehat[1], ehat[0])

    def nl(p):
        ap = (p[0] - seg_a[0], p[1] - seg_a[1])
        return ap[0] * ehat[0] + ap[1] * ehat[1], ap[0] * nhat[0] + ap[1] * nhat[1]

    along_a, h_a = nl(a)
    along_b, h_b = nl(b)
    along_p2, _ = nl(seg_b)
    e1 = max(0.0 - along_a, 0.0)
    e2 = max(along_p2 - along_a, 0.0)
    d1, d2 = (e1, e2) if e2 >= e1 else (e2, e1)
    return along_b - along_a, h_a, h_b, d1, d2


def _strip_w(f, a, b, seg_a, seg_b):
    """Modified Fresnel-zone weight w″ (Eq. 174) with edge modifiers (175/176)."""
    dprime, h_a, h_b, d1, d2 = _segment_variables(a, b, seg_a, seg_b)
    if h_a <= 0.0 or h_b <= 0.0 or abs(dprime) <= 1e-9:
        return 0.0
    lam = C0_M_S / f
    w = _fresnel_zone_w(abs(dprime), h_a, h_b, d1, d2, lam / 16.0)
    h_max = min(0.0005 * abs(b[0] - a[0]), 0.2)
    h_pp = min(h_a, h_max)
    r_first = 1.0 if h_a >= h_pp else (h_a / h_pp if h_a > 0 else 0.0)
    r_second = 1.0 if h_b >= h_max else (h_b / h_max if h_b > 0 else 0.0)
    return w * r_first * r_second


def _ff(f, dtau):
    x = abs(0.23 * math.pi * f * dtau)
    if x <= 1e-15:
        return 1.0
    return math.sin(x) / x if x <= math.pi else 0.0


def _exp_clamped(y):
    """Clamped exponential exp' (AV 1106/07 Eq. 337) — matches the engine's
    special::exp_clamped: e^y for y >= -1; e^{-1}(y+2)^2 for -2 < y < -1; 0 below."""
    if y >= -1.0:
        return math.exp(y)
    if y > -2.0:
        return math.e ** -1 * (y + 2.0) ** 2
    return 0.0


def _fc(f, cv2, ct2, t_c, rho, d):
    if cv2 == 0.0 and ct2 == 0.0:
        return 1.0
    t_abs = 273.15 + t_c
    turb = ct2 / (t_abs * t_abs) + (22.0 / 3.0) * cv2 / (C0_M_S * C0_M_S)
    x = 5.888e-3 * turb * f * f * abs(rho) ** (5.0 / 3.0) * d
    return _exp_clamped(-x)


def submodel4(f, s, r, screen, sigma, cv2, ct2, t_c, before_seg, after_seg):
    """Four-path Sub-model 4 (Eqs. 157-188), single strip per side.

    Returns (delta_l4_db, c_sr). The Fresnel-zone weight w″ (Eq. 174) and the
    Q-weight w_Q (Eq. 187) are computed exactly as the engine does — with a
    single strip per side w′ = 1 (the Eq. 188 exponent), but w_Q < 1 wherever the
    zone spills past the strip end (e.g. the source side past the screen base at
    low frequency).
    """
    w1, t, w2 = screen[0], screen[1], screen[2]
    p1 = _pwedge_pt(f, w1, t, w2, s, r)
    r_sr = _dist(s, r)
    p0 = cmath.exp(1j * 2 * math.pi * f * (r_sr / C0_M_S)) / r_sr
    p_scr = p1 / p0

    s_img, q1, dtau_s, rho1, tau2_s, zg1 = _reflect_flat(s, t, sigma, f)
    r_img, q2, dtau_r, rho2, tau2_r, zg2 = _reflect_flat(r, t, sigma, f)
    ri1 = _rho_i(zg1)
    ri2 = _rho_i(zg2)
    d1 = tau2_s * C0_M_S  # reflected path length (Fc integral length)
    d2 = tau2_r * C0_M_S

    # Fresnel-zone weights and Q-weights (Eq. 174 / 187), single strip per side.
    w1t = _strip_w(f, s, t, before_seg[0], before_seg[1])  # A=S, B=T (source side)
    w2t = _strip_w(f, t, r, after_seg[0], after_seg[1])    # A=T, B=R (receiver side)
    wq1 = 1.0 if w1t >= 1.0 else w1t * w1t
    wq2 = 1.0 if w2t >= 1.0 else w2t * w2t

    r2 = _pwedge_pt(f, w1, t, w2, s_img, r) / p1
    r3 = _pwedge_pt(f, w1, t, w2, s, r_img) / p1
    r4 = _pwedge_pt(f, w1, t, w2, s_img, r_img) / p1

    fc_s = _fc(f, cv2, ct2, t_c, rho1, d1)
    fc_r = _fc(f, cv2, ct2, t_c, rho2, d2)
    f2 = _ff(f, dtau_s) * fc_s
    f3 = _ff(f, dtau_r) * fc_r
    f4 = _ff(f, dtau_s + dtau_r) * fc_s * fc_r

    c = 1.0 + f2 * wq1 * q1 * r2 + f3 * wq2 * q2 * r3 + f4 * wq1 * wq2 * q1 * q2 * r4
    e = ((1 - f2 * f2) * abs(wq1 * ri1 * r2) ** 2
         + (1 - f3 * f3) * abs(wq2 * ri2 * r3) ** 2
         + (1 - f4 * f4) * abs(wq1 * ri1 * wq2 * ri2 * r4) ** 2)
    g = abs(c) ** 2 + e
    energy = abs(p_scr) ** 2 * g
    delta_l4 = 10.0 * math.log10(energy)
    c_sr = max(g, 1.0)  # ground part of Eq. 188 floored at 1 (Eq. 272)
    return delta_l4, c_sr


def _table_interp(table, col_val, row_val):
    def frac(v, axis):
        if v <= axis[0]:
            return 0, 0, 0.0
        if v >= axis[-1]:
            return len(axis) - 1, len(axis) - 1, 0.0
        i = 0
        while i + 1 < len(axis) and axis[i + 1] < v:
            i += 1
        return i, i + 1, (v - axis[i]) / (axis[i + 1] - axis[i])
    r0, r1, tr = frac(row_val, ROW_AXIS)
    c0, c1, tc = frac(col_val, COL_AXIS)
    top = table[r0][c0] * (1 - tc) + table[r0][c1] * tc
    bot = table[r1][c0] * (1 - tc) + table[r1][c1] * tc
    return top * (1 - tr) + bot * tr


def _heaviside(x):
    return 1.0 if x > 0.0 else 0.0


def _scatter_channel(table, f, r1, r2, he, ce2, c_sr, f0):
    if ce2 <= 0.0:
        return NO_SCATTER_DB
    l_a = _table_interp(table, 40 * r2 / r1, 40 * he / r1) + 10 * math.log10(r1 / 40)
    l_b = _table_interp(table, 40 * r1 / r2, 40 * he / r2) + 10 * math.log10(r2 / 40)
    l0 = max(l_a, l_b)
    return (l0 + 10 * math.log10(c_sr) + (10.0 / 3.0) * math.log10(f / 2000.0)
            + 10 * math.log10(ce2)
            + 15 * _heaviside(f0 - f) * math.log10(f / f0)
            + 15 * _heaviside(0.5 * f0 - f) * math.log10(f / (0.5 * f0)))


def submodel7(f, r1, r2, he, cv2, ct2, c_sr, t_c):
    """Turbulence-scattering ΔL₇ (Eqs. 271-274). he>0 required."""
    if he <= 0.0:
        return NO_SCATTER_DB
    if cv2 == 0.0 and ct2 == 0.0:
        return NO_SCATTER_DB
    cve2 = 10.0 * cv2  # Eq. 271 deliberate x10
    cte2 = 10.0 * ct2
    c_sr = max(c_sr, 1.0)
    theta = math.pi - math.atan(r1 / he) - math.atan(r2 / he)
    c0 = 20.05 * math.sqrt(t_c + 273.15)  # Coft(t)
    sh = math.sin(theta / 2.0)
    f0 = c0 / (2.0 * sh) if abs(sh) > 1e-12 else f
    dl_ws = _scatter_channel(TABLE6, f, r1, r2, he, cve2, c_sr, f0)
    dl_ts = _scatter_channel(TABLE7, f, r1, r2, he, cte2, c_sr, f0)
    return 10.0 * math.log10(10 ** (dl_ws / 10) + 10 ** (dl_ts / 10))


def _combine(dl4, dl7):
    return 10.0 * math.log10(10 ** (dl4 / 10) + 10 ** (dl7 / 10))


def _freq_axis():
    """ENVI 105-point 1/12-octave grid: f(x) = 1000·G^(x/12), x = -64..40."""
    g = 10.0 ** 0.3
    return [1000.0 * g ** (x / 12.0) for x in range(-64, 41)]


def _fmt(x):
    return repr(float(x))


def main():
    # Literal case-71 thin-screen geometry (RESEARCH §14): spike at x=15.
    s = (0.0, 0.5)
    r = (150.0, 1.5)
    screen = [(14.99, 0.0), (15.0, 4.0), (15.01, 0.0)]
    sigma = 200.0
    cv2, ct2, t_c = 0.12, 0.008, 15.0
    # Wide flat reflecting strips (same as the harness test): the source-side
    # strip ends at the screen base x=15, so at low f the zone spills past it.
    before_seg = ((-500.0, 0.0), (15.0, 0.0))
    after_seg = ((15.0, 0.0), (500.0, 0.0))
    # SM7 geometry: R1 = S->edge, R2 = edge->R, h_e = edge above the S-R line.
    r1_sm7 = screen[1][0] - s[0]
    r2_sm7 = r[0] - screen[1][0]
    slope = (r[1] - s[1]) / (r[0] - s[0])
    h_e = screen[1][1] - (s[1] + slope * (screen[1][0] - s[0]))

    lines = ["# Nord2000 screen-over-ground oracle: case-71 thin screen, ΔL₄+ΔL₇.",
             "# GENERATED by tools/nord2000_oracle/gen_screen_fixtures.py -- DO NOT EDIT.",
             "# Independent SM4 four-path (Eqs. 157-188) + SM7 (Eqs. 271-274, Tables 6/7);",
             "# wedge-face Q̂ via scipy.special.wofz (common.py). Wide strips -> w_Q=1.",
             ""]
    src = ((Path(__file__).parent / "common.py").read_bytes()
           + (Path(__file__).parent / "gen_wedge_fixtures.py").read_bytes())
    prov = hashlib.sha256(src).hexdigest()[:16]
    lines += ["[meta]",
              f'provenance = "common+wedge sha256:{prov}"',
              "tol_db = 0.1",
              f"s_x = {_fmt(s[0])}", f"s_z = {_fmt(s[1])}",
              f"r_x = {_fmt(r[0])}", f"r_z = {_fmt(r[1])}",
              f"edge_x = {_fmt(screen[1][0])}", f"edge_z = {_fmt(screen[1][1])}",
              f"sigma_kpa = {_fmt(sigma)}",
              f"cv2 = {_fmt(cv2)}", f"ct2 = {_fmt(ct2)}", f"t_air_c = {_fmt(t_c)}",
              f"r1_sm7 = {_fmt(r1_sm7)}", f"r2_sm7 = {_fmt(r2_sm7)}", f"h_e = {_fmt(h_e)}",
              ""]

    for f in _freq_axis():
        dl4, c_sr = submodel4(f, s, r, screen, sigma, cv2, ct2, t_c, before_seg, after_seg)
        dl7 = submodel7(f, r1_sm7, r2_sm7, h_e, cv2, ct2, c_sr, t_c)
        combined = _combine(dl4, dl7)
        lines += ["[[point]]",
                  f"f = {_fmt(f)}",
                  f"delta_l4 = {_fmt(dl4)}",
                  f"delta_l7 = {_fmt(dl7)}",
                  f"delta_l = {_fmt(combined)}",
                  ""]

    out = (Path(__file__).resolve().parents[2]
           / "crates/envi-harness/tests/fixtures/oracle/screen_thin.toml")
    out.write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote {out} ({len(_freq_axis())} points)")


if __name__ == "__main__":
    main()
