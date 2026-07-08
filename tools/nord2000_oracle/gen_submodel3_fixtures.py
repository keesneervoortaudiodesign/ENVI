"""Generate the committed Sub-model 3 (non-flat terrain) oracle fixture.

Writes ``crates/envi-harness/tests/fixtures/oracle/submodel3.toml`` — a
cross-implementation reference for the ENVI Rust engine's
``propagation::terrain_effect::submodel3`` (AV 1106/07 §5.12, Eqs. 134-156).
Regeneration is operator-driven (``python gen_submodel3_fixtures.py``); the TOML
is committed and Python/scipy are NOT build dependencies — Rust tests run
against the committed data alone.

Scope: the CONCAVE path (Eqs. 134-140, 156) over horizontal ground segments,
where the reflection geometry reduces to the flat two-ray model with
segment-relative heights — so this reuses the ``common.py`` ground/Q-hat oracle
and ``flat_models`` Fresnel machinery rather than re-deriving the sloped-segment
reflection. The convex/wedge path (Eqs. 141-151) is NOT implemented in the
engine (a typed error) and is therefore not fixtured here.

Standing caveat (04-RESEARCH): this oracle re-transcribes the same equations as
the engine, so it cross-checks the IMPLEMENTATION, not the spec reading. The
FORCE .xls remains the only external authority. Equations are cited by report +
number only (licensing rule, CLAUDE.md).
"""

from __future__ import annotations

import hashlib
import math
from pathlib import Path

import common
import flat_models as fm

F_LAMBDA = 1.0 / 16.0  # Sub-model 3 Fresnel fraction (Eq. 134 / 375).


def submodel1_channels(f_hz, d, h_s, h_r, sigma, c0):
    """Two-channel Sub-model 1 over the flat (segment-relative) geometry."""
    rays = fm.straight_rays(d, h_s, h_r, c0)
    z_g = common.ground_impedance(f_hz, sigma)
    q = common.spherical_q(f_hz, rays["tau2"], rays["psi_g"], z_g)
    ratio = rays["r1"] / rays["r2"]
    rho_sep = 2.0 * h_s * h_r / (h_s + h_r) if (h_s + h_r) > 0 else 0.0
    f_coh = fm.coherence_f(f_hz, rays["dtau"], rho_sep, 0.0, 0.0, 15.0, c0)
    phase = complex(
        math.cos(2 * math.pi * f_hz * rays["dtau"]),
        math.sin(2 * math.pi * f_hz * rays["dtau"]),
    )
    h_coh = 1.0 + f_coh * ratio * phase * q
    rho_i = fm.incoherent_rho(z_g)
    p_incoh = (1.0 - f_coh * f_coh) * (ratio * rho_i) ** 2
    return h_coh, p_incoh


def min_concave_height(h_other, d1, d, lam):
    """MinConcaveHeight (Eqs. 373-375)."""
    h = abs(h_other)
    d2 = d - d1
    if math.hypot(d2, h) - h <= F_LAMBDA * lam:
        return math.inf
    x1 = (math.hypot(h, d2) - F_LAMBDA * lam) ** 2
    x2 = h * h + d * d - d1 * d1 - x1
    a = 4.0 * h * h - 4.0 * x1
    b = 4.0 * h * x2
    c = x2 * x2 - 4.0 * d1 * d1 * x1
    if abs(a) < 1e-30:
        return math.inf
    disc = b * b - 4.0 * a * c
    if disc < 0.0:
        return math.inf
    val = (-b - math.sqrt(disc)) / (2.0 * a)
    return val if (math.isfinite(val) and val > 0.0) else math.inf


def submodel3_delta_l(f_hz, source, receiver, segments, c0):
    """Sub-model 3 concave two-channel result (Eqs. 134-140, 156)."""
    lam = c0 / f_hz
    flp = F_LAMBDA * lam
    w, dl, hc, pi = [], [], [], []
    for (ax, az, bx, bz, sigma, _rough) in segments:
        # Horizontal segment: segment-relative heights are z - z_seg; along-x is
        # x - ax. (Sloped segments are out of this oracle's horizontal scope.)
        assert abs(az - bz) < 1e-12, "oracle fixtures use horizontal segments only"
        d = receiver[0] - source[0]
        h_s = source[1] - az
        h_r = receiver[1] - az
        d1 = source[0] - ax  # source foot at along = 0 ⇒ segment start at -(a_s)
        d1_frame = -(source[0] - ax)
        d2_frame = (bx - ax) - (source[0] - ax)
        # Classification via relative heights (Eqs. 136-139).
        hs_fz = min_concave_height(h_r, abs(d1_frame), abs(d), lam)
        hr_fz = min_concave_height(h_s, abs(d - d2_frame), abs(d), lam)
        hs_dd = min(abs(h_s), hs_fz)
        hr_dd = min(abs(h_r), hr_fz)
        rel_s = 1.0 if h_s >= hs_dd else (h_s / hs_dd if h_s > 0 else 0.0)
        rel_r = 1.0 if h_r >= hr_dd else (h_r / hr_dd if h_r > 0 else 0.0)
        assert rel_s >= 1.0 - 1e-12 and rel_r >= 1.0 - 1e-12, "oracle configs are concave"
        h_coh, p_incoh = submodel1_channels(f_hz, d, h_s, h_r, sigma, c0)
        wi = fm.fresnel_zone_w(d, h_s, h_r, d1_frame, d2_frame, flp) if (h_s > 0 and h_r > 0) else 0.0
        w.append(wi)
        dl.append(10.0 * math.log10(abs(h_coh) ** 2 + p_incoh))
        hc.append(h_coh)
        pi.append(p_incoh)
    w_t = sum(w)
    scale = 2.0 / w_t if w_t > 2.0 else 1.0
    wp = [wi * scale for wi in w]
    sum_wp = sum(wp)
    dl0 = sum(a * b for a, b in zip(wp, dl))
    if dl0 >= 0.0:
        r_prime = 0.0
    elif dl0 <= -20.0:
        r_prime = 1.0
    else:
        r_prime = -dl0 / 20.0
    factor = (1.0 - r_prime + r_prime / sum_wp) if sum_wp > 1e-12 else 1.0
    return dl0 * factor


# Concave test configurations: horizontal floor segments below an elevated
# source and receiver (valley / stepped-valley shapes). All segments concave.
CONFIGS = [
    {
        "name": "single_floor",
        "source": [0.0, 4.0],
        "receiver": [200.0, 4.0],
        # (ax, az, bx, bz, sigma_kpa, roughness)
        "segments": [(-100.0, -2.0, 300.0, -2.0, 200.0, 0.0)],
    },
    {
        "name": "stepped_valley",
        "source": [0.0, 5.0],
        "receiver": [240.0, 5.0],
        "segments": [
            (-50.0, -1.0, 80.0, -1.0, 20000.0, 0.0),
            (80.0, -3.0, 160.0, -3.0, 200.0, 0.0),
            (160.0, -1.0, 300.0, -1.0, 200.0, 0.0),
        ],
    },
]

# Every 8th grid point (13 bands from 25 Hz to 10 kHz) — enough to exercise the
# low/mid/high behaviour without an oversized fixture.
BAND_INDICES = list(range(0, 105, 8))


def main() -> None:
    axis = fm.freq_axis()
    c0 = common.C0_M_S
    provenance = hashlib.sha256(
        (Path(__file__).with_name("common.py")).read_bytes()
    ).hexdigest()

    out = Path(__file__).resolve().parents[2] / (
        "crates/envi-harness/tests/fixtures/oracle/submodel3.toml"
    )
    lines = [
        "# generated by tools/nord2000_oracle/gen_submodel3_fixtures.py — DO NOT EDIT",
        "# Sub-model 3 concave path (AV 1106/07 §5.12, Eqs. 134-140, 156).",
        "",
        "[meta]",
        'oracle = "scipy wofz + AV 1106/07 §5.12 transcription (concave, horizontal segments)"',
        f'provenance = "common.py sha256:{provenance}"',
        "# Cross-implementation tolerance: the ground/Q-hat oracle intrinsic error",
        "# (~2.5e-6, three-pole w(z) near-border) dominates; 0.02 dB is comfortable.",
        "tol_abs_db = 0.02",
        "",
    ]
    for cfg in CONFIGS:
        src = cfg["source"]
        rcv = cfg["receiver"]
        segs = cfg["segments"]
        seg_a = [[s[0], s[1]] for s in segs]
        seg_b = [[s[2], s[3]] for s in segs]
        sigma = [s[4] for s in segs]
        rough = [s[5] for s in segs]
        bands = []
        vals = []
        for bi in BAND_INDICES:
            f = axis[bi]
            bands.append(bi)
            vals.append(submodel3_delta_l(f, src, rcv, segs, c0))
        lines.append("[[case]]")
        lines.append(f'name = "{cfg["name"]}"')
        lines.append(f"source = [{src[0]}, {src[1]}]")
        lines.append(f"receiver = [{rcv[0]}, {rcv[1]}]")
        lines.append(f"seg_a = {seg_a}")
        lines.append(f"seg_b = {seg_b}")
        lines.append(f"sigma = {sigma}")
        lines.append(f"rough = {rough}")
        lines.append(f"band_index = {bands}")
        lines.append("delta_l_db = [" + ", ".join(f"{v:.10f}" for v in vals) + "]")
        lines.append("")

    out.write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
