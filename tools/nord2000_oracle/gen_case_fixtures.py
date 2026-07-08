"""Generate the committed end-to-end terrain-effect case fixtures (Phase 2).

Composes the independent scipy-based sub-model oracles into the full §5.22
Eq. 332 terrain effect ΔL_t and writes five oracle-pinned 105-point references
the Rust engine's ``propagation::terrain_effect::terrain_effect`` is cross-checked
against through the harness ``CaseKind::Terrain`` path:

  * ``terrain_flat_sigma200.toml``       — flat σ=200, Sub-model 1 (r_scr1 = 0).
  * ``terrain_mixed_case21.toml``        — FORCE case-21 mixed ground, Sub-model 2.
  * ``terrain_screen_thin_case71.toml``  — thin screen, Sub-model 4 + 7.
  * ``terrain_screen_thick_case81.toml`` — thick screen, Sub-model 5 + 7.
  * ``terrain_screens_double_case91.toml``— two screens, Sub-model 6 + 7.

Independence: the Faddeeva ``w(z)`` and the wedge kernels come from
``scipy.special.wofz`` (``common.py``) and the independent wedge oracle
(``gen_wedge_fixtures``); the flat sub-models from ``flat_models``; the four/eight
-path screen composition and Sub-model 7 from ``gen_screen_fixtures``. The §5.21
transition parameter ``r_scr1`` (Eqs. 301-305) and the Eq. 332 blend are
re-implemented here from the same page-image reading the engine uses. Equations
cited by AV 1106/07 report + number only (CLAUDE.md licensing). Developer tool
emitting committed fixture DATA; NOT a build dependency.

Nord2000-native convention: time e^{-jwt}, outgoing phase e^{+jwt}.
"""

from __future__ import annotations

import cmath
import hashlib
import math
from pathlib import Path

import common
import flat_models as fm
import gen_screen_fixtures as gs
from gen_wedge_fixtures import HARD, p2edge, p2wedge, pwedge

C0 = common.C0_M_S
FRAC_PI_2 = math.pi / 2.0


def _dist(a, b):
    return math.hypot(b[0] - a[0], b[1] - a[1])


def _freq_axis():
    g = 1.9952623149688795  # 10^0.3
    return [1000.0 * g ** ((i - 64) / 12.0) for i in range(105)]


# --------------------------------------------------------------------------- #
# Wedge angles + kernels (mirror envi-engine screen.rs wedge_angles/two_wedge). #
# --------------------------------------------------------------------------- #
def _wedge_angles(w1, t, w2, s, r):
    b1 = math.atan((w1[1] - t[1]) / (t[0] - w1[0])) + FRAC_PI_2
    b2 = math.atan((w2[1] - t[1]) / (w2[0] - t[0])) + FRAC_PI_2
    th1 = math.atan((s[1] - t[1]) / (t[0] - s[0])) + FRAC_PI_2
    th2 = math.atan((r[1] - t[1]) / (r[0] - t[0])) + FRAC_PI_2
    return (2 * math.pi - b1 - b2, 2 * math.pi - th1 - b2, th2 - b2)


def _two_wedge_geom(s, t1, t2, r, w1, w2):
    beta1, th1s, th1r = _wedge_angles(w1, t1, t2, s, t2)
    beta2, th2s, th2r = _wedge_angles(t1, t2, w2, t1, r)
    rs = _dist(s, t1)
    rm = _dist(t1, t2)
    rr = _dist(t2, r)
    return {
        "beta1": beta1, "theta_1s": th1s, "theta_1r": th1r,
        "beta2": beta2, "theta_2s": th2s, "theta_2r": th2r,
        "tau_s": rs / C0, "tau_m": rm / C0, "tau_r": rr / C0,
        "r_s": rs, "r_m": rm, "r_r": rr,
    }


def _pwedge_kernel(w1, t, w2):
    def diffract(f, s, r):
        beta, ts, tr = _wedge_angles(w1, t, w2, s, r)
        rs = _dist(s, t)
        rr = _dist(t, r)
        return pwedge(f, beta, ts, tr, (rs + rr) / C0, rs / C0, rr / C0,
                      rs + rr, rs, rr, HARD, HARD)
    return diffract


def _p2edge_kernel(w1, t1, t2, w2):
    def diffract(f, s, r):
        g = _two_wedge_geom(s, t1, t2, r, w1, w2)
        return p2edge(f, g, "first", HARD, HARD)
    return diffract


def _p2wedge_kernel(w1, t1, t2, w2):
    def diffract(f, s, r):
        g = _two_wedge_geom(s, t1, t2, r, w1, w2)
        return p2wedge(f, g, "first", HARD, HARD, HARD, HARD)
    return diffract


# --------------------------------------------------------------------------- #
# Four-path (SM4/SM5) + eight-path (SM6) composition — mirror screen.rs.        #
# --------------------------------------------------------------------------- #
class _Side:
    __slots__ = ("q", "ri", "dtau", "rho_sep", "d_prime", "image")

    def __init__(self, f, endpoint, top, seg, sigma, cv2, ct2, t_c):
        image, q, dtau, rho_sep, tau2, zg = gs._reflect_flat(endpoint, top, sigma, f)
        self.q = q
        self.ri = gs._rho_i(zg)
        self.dtau = dtau
        self.rho_sep = rho_sep
        self.d_prime = tau2 * C0
        self.image = image


def _four_path(f, s, r, top, diffract, before_seg, after_seg, sigma, cv2, ct2, t_c):
    """Four-path screen⇄ground model (Eqs. 157-188), wide single strip per side."""
    p1 = diffract(f, s, r)
    r_sr = _dist(s, r)
    p0 = cmath.exp(1j * 2 * math.pi * f * (r_sr / C0)) / r_sr
    p_scr = p1 / p0

    sv = _Side(f, s, top, before_seg, sigma, cv2, ct2, t_c)
    rv = _Side(f, r, top, after_seg, sigma, cv2, ct2, t_c)
    # w_Q from the actual Fresnel-zone coverage (matches the engine).
    w1t = gs._strip_w(f, s, top, before_seg[0], before_seg[1])
    w2t = gs._strip_w(f, top, r, after_seg[0], after_seg[1])
    wq1 = 1.0 if w1t >= 1.0 else w1t * w1t
    wq2 = 1.0 if w2t >= 1.0 else w2t * w2t

    r2 = diffract(f, sv.image, r) / p1
    r3 = diffract(f, s, rv.image) / p1
    r4 = diffract(f, sv.image, rv.image) / p1

    fc_s = gs._fc(f, cv2, ct2, t_c, sv.rho_sep, sv.d_prime)
    fc_r = gs._fc(f, cv2, ct2, t_c, rv.rho_sep, rv.d_prime)
    f2 = gs._ff(f, sv.dtau) * fc_s
    f3 = gs._ff(f, rv.dtau) * fc_r
    f4 = gs._ff(f, sv.dtau + rv.dtau) * fc_s * fc_r

    c = (1.0 + f2 * wq1 * sv.q * r2 + f3 * wq2 * rv.q * r3
         + f4 * wq1 * wq2 * sv.q * rv.q * r4)
    e = ((1 - f2 * f2) * abs(wq1 * sv.ri * r2) ** 2
         + (1 - f3 * f3) * abs(wq2 * rv.ri * r3) ** 2
         + (1 - f4 * f4) * abs(wq1 * sv.ri * wq2 * rv.ri * r4) ** 2)
    g = abs(c) ** 2 + e
    delta_l = 10.0 * math.log10(abs(p_scr) ** 2 * g)
    return delta_l, max(g, 1.0)


def _eight_path(f, s, r, t1, t2, diffract, before_seg, middle_seg, after_seg,
                sigma, cv2, ct2, t_c):
    """Eight-ray two-screen model (Eq. 222), wide single strip per region."""
    p1 = diffract(f, s, r)
    r_sr = _dist(s, r)
    p0 = cmath.exp(1j * 2 * math.pi * f * (r_sr / C0)) / r_sr
    p_scr = p1 / p0

    before = _Side(f, s, t1, before_seg, sigma, cv2, ct2, t_c)
    after = _Side(f, r, t2, after_seg, sigma, cv2, ct2, t_c)
    middle = _Side(f, r, t2, middle_seg, sigma, cv2, ct2, t_c)

    coherent = 0.0 + 0.0j
    incoh = 0.0
    for mask in range(8):
        ub, um, ua = mask & 1, mask & 2, mask & 4
        sp = before.image if ub else s
        rp = after.image if ua else (middle.image if um else r)
        ratio = diffract(f, sp, rp) / p1
        q = 1.0 + 0.0j
        rho = 1.0
        dtau = 0.0
        rho_sep = 0.0
        d_prime = 0.0
        n = 0
        for used, v in ((ub, before), (um, middle), (ua, after)):
            if used:
                q *= v.q
                rho *= v.ri
                dtau += v.dtau
                rho_sep += v.rho_sep
                d_prime += v.d_prime
                n += 1
        if n == 0:
            coherent += ratio
            continue
        fcv = gs._fc(f, cv2, ct2, t_c, rho_sep / n, d_prime)
        fcoh = gs._ff(f, dtau) * fcv
        coherent += fcoh * q * ratio
        incoh += (1 - fcoh * fcoh) * (rho * abs(ratio)) ** 2

    hc = p_scr * coherent
    p_incoh = abs(p_scr) ** 2 * incoh
    return 10.0 * math.log10(abs(hc) ** 2 + p_incoh), 1.0


# --------------------------------------------------------------------------- #
# §5.21 transition parameter r_scr1 (Eqs. 301-305) + Sub-model 7 + Eq. 332.     #
# --------------------------------------------------------------------------- #
def _ramp(v, lo, hi):
    if v <= lo:
        return 0.0
    if v >= hi:
        return 1.0
    return (v - lo) / (hi - lo)


def _r_scr1(f, s, r, edge):
    lam = C0 / f
    d0 = _dist(s, edge) + _dist(edge, r) - _dist(s, r)
    ratio = d0 / lam
    r_dl = 1.0 if ratio >= 0 else (1.0 + ratio / 0.133 if ratio > -0.133 else 0.0)
    h_scr = edge[1]  # baseline z = 0
    r_h = _ramp(h_scr / lam, 0.1, 0.3)
    rs, rr = _dist(s, edge), _dist(edge, r)
    hfz = fm.calc_fz_d(rs, rr, math.pi / 2.0, 0.5 * lam)
    r_fz = _ramp(h_scr / hfz, 0.026, 0.082) if hfz > 0 else 1.0
    return max(0.0, min(1.0, r_dl * r_h * r_fz))


def _combine(a_db, b_db):
    return 10.0 * math.log10(10 ** (a_db / 10) + 10 ** (b_db / 10))


def _sm7(f, s, r, edge, cv2, ct2, c_sr, t_c):
    r1 = edge[0] - s[0]
    r2 = r[0] - edge[0]
    slope = (r[1] - s[1]) / (r[0] - s[0])
    h_e = edge[1] - (s[1] + slope * (edge[0] - s[0]))
    return gs.submodel7(f, abs(r1), abs(r2), h_e, cv2, ct2, c_sr, t_c)


def _flat_sm1_db(f, d, h_s, h_r, sigma, cv2, ct2, t_c):
    fm.HEIGHTS = (h_s, h_r)
    fm.DIST_D = d
    rays = fm.straight_rays(d, h_s, h_r, C0)
    h_coh, p_incoh = fm.submodel1(f, rays, sigma, C0, cv2, ct2, t_c)
    return fm.delta_l_db(h_coh, p_incoh)


# --------------------------------------------------------------------------- #
# Fixtures.                                                                     #
# --------------------------------------------------------------------------- #
def _fmt(x):
    return repr(float(x))


def _provenance():
    here = Path(__file__).resolve().parent
    h = hashlib.sha256()
    for name in ("common.py", "flat_models.py", "gen_screen_fixtures.py",
                 "gen_wedge_fixtures.py", "gen_case_fixtures.py"):
        h.update((here / name).read_bytes())
    return h.hexdigest()[:16]


def _write(out_dir, name, header, src, rcv, cv2, ct2, rows, bands):
    """Emit a complete terrain CaseKind::Terrain TOML (geometry + rows + values)."""
    lines = [
        "# GENERATED by tools/nord2000_oracle/gen_case_fixtures.py -- DO NOT EDIT.",
        "# Oracle-pinned end-to-end terrain effect ΔL_t (AV 1106/07 §5.21-5.22).",
        "# tolerance_db = 0.1: cross-implementation gate (02-RESEARCH acceptance",
        "# ladder rung 2). scipy.special.wofz Faddeeva; provenance below.",
        "",
        "[meta]",
        f'name = "{header}"',
        'kind = "terrain"',
        'reference = "analytic"',
        f'provenance = "oracle sha256:{_provenance()}"',
        "",
        "[source]",
        f"position = [{_fmt(src[0])}, {_fmt(src[1])}, {_fmt(src[2])}]",
        "",
        "[receiver]",
        f"position = [{_fmt(rcv[0])}, {_fmt(rcv[1])}, {_fmt(rcv[2])}]",
        "",
        "[atmosphere]",
        "t_air_c = 15.0",
        f"cv2 = {_fmt(cv2)}",
        f"ct2 = {_fmt(ct2)}",
        "",
    ]
    for (x, z, sigma) in rows:
        lines += ["[[terrain]]",
                  f"x = {_fmt(x)}", f"z = {_fmt(z)}", f"sigma_kpa = {_fmt(sigma)}",
                  "roughness = 0.0", ""]
    lines += ["[expected]", "tolerance_db = 0.1",
              'bands = "oracle:terrain_effect"', "values = ["]
    lines += [f"  {_fmt(v)}," for v in bands]
    lines += ["]", ""]
    (out_dir / name).write_text("\n".join(lines), encoding="utf-8")


def main():
    here = Path(__file__).resolve().parent
    root = here.parents[1]
    out_dir = root / "cases"
    out_dir.mkdir(parents=True, exist_ok=True)
    axis = _freq_axis()

    # 1) Flat σ=200 (Sub-model 1, r_scr1 = 0). hS=0.5, hR=1.5, d=97.5.
    bands = [_flat_sm1_db(f, 97.5, 0.5, 1.5, 200.0, 0.0, 0.0, 15.0) for f in axis]
    _write(out_dir, "terrain_flat_sigma200.toml",
           "flat ground sigma=200, hs=0.5 hr=1.5 d=97.5 (Sub-model 1)",
           (2.5, 0.0, 0.5), (100.0, 0.0, 1.5), 0.0, 0.0,
           [(2.5, 0.0, 200.0), (100.0, 0.0, 200.0)], bands)

    # 2) FORCE case-21 mixed ground (Sub-model 2), Cv²=0.12, CT²=0.008.
    strips21 = [
        {"x0": 0.0, "x1": 10.0, "sigma": 20000.0, "rough": 0.0},
        {"x0": 10.0, "x1": 50.0, "sigma": 200.0, "rough": 0.0},
        {"x0": 50.0, "x1": 60.0, "sigma": 20000.0, "rough": 0.0},
        {"x0": 60.0, "x1": 97.5, "sigma": 200.0, "rough": 0.0},
    ]
    fm.HEIGHTS = (0.5, 1.5)
    fm.DIST_D = 97.5
    bands = []
    for f in axis:
        h_coh, p_incoh = fm.submodel2(f, strips21, 97.5, 0.5, 1.5, C0, 0.12, 0.008, 15.0)
        bands.append(fm.delta_l_db(h_coh, p_incoh))
    _write(out_dir, "terrain_mixed_case21.toml",
           "FORCE case-21 mixed road/grass ground (Sub-model 2)",
           (2.5, 0.0, 0.5), (100.0, 0.0, 1.5), 0.12, 0.008,
           [(2.5, 0.0, 20000.0), (12.5, 0.0, 200.0), (52.5, 0.0, 20000.0),
            (62.5, 0.0, 200.0), (100.0, 0.0, 200.0)], bands)

    cv2, ct2, t_c = 0.12, 0.008, 15.0

    # 3) Thin screen (Sub-model 4 + 7). Spike 4 m at x=15 on flat σ=200 ground.
    s, r = (0.0, 0.5), (150.0, 1.5)
    edge = (15.0, 4.0)
    w1, w2 = (14.99, 0.0), (15.01, 0.0)
    before = ((s[0] - 500.0, 0.0), (edge[0], 0.0))
    after = ((edge[0], 0.0), (r[0] + 500.0, 0.0))
    diff = _pwedge_kernel(w1, edge, w2)
    bands = []
    for f in axis:
        dl4, c_sr = _four_path(f, s, r, edge, diff, before, after, 200.0, cv2, ct2, t_c)
        dl7 = _sm7(f, s, r, edge, cv2, ct2, c_sr, t_c)
        dl_scr = _combine(dl4, dl7)
        dl_flat = _flat_sm1_db(f, r[0] - s[0], s[1], r[1], 200.0, cv2, ct2, t_c)
        rr = _r_scr1(f, s, r, edge)
        bands.append(rr * dl_scr + (1 - rr) * dl_flat)
    _write(out_dir, "terrain_screen_thin_case71.toml",
           "FORCE case-71 thin screen (Sub-model 4 + 7)",
           (0.0, 0.0, 0.5), (150.0, 0.0, 1.5), cv2, ct2,
           [(0.0, 0.0, 200.0), (14.99, 0.0, 200.0), (15.0, 4.0, 200.0),
            (15.01, 0.0, 200.0), (150.0, 0.0, 200.0)], bands)

    # 4) Thick screen (Sub-model 5 + 7). Flat top 15->30 m at h=2 m.
    s, r = (0.0, 0.5), (150.0, 1.5)
    t1, t2 = (15.0, 2.0), (30.0, 2.0)
    edge = t1  # primary edge (screen[1]) for r_scr1 + SM7
    w1, w2 = (14.99, 0.0), (30.01, 0.0)
    edge_x = 0.5 * (t1[0] + t2[0])
    before = ((s[0] - 500.0, 0.0), (edge_x, 0.0))
    after = ((edge_x, 0.0), (r[0] + 500.0, 0.0))
    diff = _p2edge_kernel(w1, t1, t2, w2)
    bands = []
    for f in axis:
        dl5, _ = _four_path(f, s, r, t1, diff, before, after, 200.0, cv2, ct2, t_c)
        dl7 = _sm7(f, s, r, edge, cv2, ct2, 1.0, t_c)
        dl_scr = _combine(dl5, dl7)
        dl_flat = _flat_sm1_db(f, r[0] - s[0], s[1], r[1], 200.0, cv2, ct2, t_c)
        rr = _r_scr1(f, s, r, edge)
        bands.append(rr * dl_scr + (1 - rr) * dl_flat)
    _write(out_dir, "terrain_screen_thick_case81.toml",
           "FORCE case-81 thick screen (Sub-model 5 + 7)",
           (0.0, 0.0, 0.5), (150.0, 0.0, 1.5), cv2, ct2,
           [(0.0, 0.0, 200.0), (14.99, 0.0, 200.0), (15.0, 2.0, 200.0),
            (30.0, 2.0, 200.0), (30.01, 0.0, 200.0), (150.0, 0.0, 200.0)], bands)

    # 5) Two screens (Sub-model 6 + 7). Spike at 15 + trapezoid 75-85 m.
    s, r = (0.0, 0.5), (150.0, 1.5)
    t1, t2 = (15.0, 4.0), (80.0, 3.0)
    edge = t1  # primary (tallest) edge
    w1 = (14.99, 0.0)
    w2 = (85.0, 0.0)
    midx = 0.5 * (15.01 + 75.0)
    before = ((s[0] - 500.0, 0.0), (t1[0], 0.0))
    middle = ((t1[0], 0.0), (midx, 0.0))
    after = ((t2[0], 0.0), (r[0] + 500.0, 0.0))
    diff = _p2wedge_kernel(w1, t1, t2, w2)
    bands = []
    for f in axis:
        dl6, _ = _eight_path(f, s, r, t1, t2, diff, before, middle, after,
                             200.0, cv2, ct2, t_c)
        dl7 = _sm7(f, s, r, edge, cv2, ct2, 1.0, t_c)
        dl_scr = _combine(dl6, dl7)
        dl_flat = _flat_sm1_db(f, r[0] - s[0], s[1], r[1], 200.0, cv2, ct2, t_c)
        rr = _r_scr1(f, s, r, edge)
        bands.append(rr * dl_scr + (1 - rr) * dl_flat)
    _write(out_dir, "terrain_screens_double_case91.toml",
           "FORCE case-91 two screens (Sub-model 6 + 7)",
           (0.0, 0.0, 0.5), (150.0, 0.0, 1.5), cv2, ct2,
           [(0.0, 0.0, 200.0), (14.99, 0.0, 200.0), (15.0, 4.0, 200.0),
            (15.01, 0.0, 200.0), (75.0, 0.0, 200.0), (80.0, 3.0, 200.0),
            (85.0, 0.0, 200.0), (150.0, 0.0, 200.0)], bands)

    print(f"wrote 5 terrain case fixtures to {out_dir} ({len(axis)} points each)")


if __name__ == "__main__":
    main()
