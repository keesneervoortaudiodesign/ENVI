"""Generate the committed forest (Sub-model 10) excess-attenuation oracle fixture.

Writes ``crates/envi-harness/tests/fixtures/oracle/forest.toml`` — a
cross-implementation reference for the ENVI Rust engine's
``envi_engine::forest::forest_delta_l`` (AV 1106/07 §5.19 Sub-Model 10,
Eqs. 288-291 and Tables 8/9). Regeneration is operator-driven
(``python gen_forest_fixtures.py``); the TOML is committed and Python/numpy/scipy
are NOT build dependencies — the Rust test runs against the committed data alone.

Scope: the excess attenuation ``ΔL_s = Max(1.25·k_f·T·A_e, −15)`` with
``T = Min((R_sc·nQ/1.75)², 1)``, ``nQ = 2·a·n″``,
``A_e = ΔL(h′, α, R′) + 20·log₁₀(8·R′)`` — Table 8 (k_f) by linear interpolation
(numpy ``interp``), Table 9 (ΔL) by tensor-product PCHIP
(``scipy.interpolate.PchipInterpolator``) with the IDENTICAL nesting order as the
engine (R′ → α → log₁₀(h′)) and the same edge clamps. The Fs coherence factor
(Eq. 288) is deferred in Phase 5 and NOT part of this oracle.

Standing caveat (05-RESEARCH): this oracle re-transcribes the same equations and
the same interpolation scheme as the engine, so it cross-checks the
IMPLEMENTATION (table indices, nesting order, clamps, the FC-Butland PCHIP), not
the spec reading. The node-exact analytic anchors (F1/F2) pin the spec reading
independently. Equations cited by report + number only (licensing rule).
"""

from __future__ import annotations

import hashlib
import math
from pathlib import Path

import numpy as np
from scipy.interpolate import PchipInterpolator

import common
import flat_models as fm

# --- Table 8: k_f(ka), linear interpolation, edge-clamped (A2). --------------
KA_AXIS = [0.0, 0.7, 1.0, 1.5, 3.0, 5.0, 10.0, 20.0]
KF_VALS = [0.00, 0.00, 0.05, 0.20, 0.70, 0.82, 0.95, 1.00]

# --- Table 9 axes and data, indexed [h_idx][alpha_idx][r_idx]. ---------------
R_NORM_AXIS = [0.0625, 0.125, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 10.0]
H_NORM_AXIS = [0.01, 0.1, 1.0]
ALPHA_AXIS = [0.0, 0.2, 0.4]
TABLE9 = [
    # h' = 0.01
    [
        [6.0, 0.0, -7.5, -14.0, -18.0, -21.5, -26.3, -31.0, -40.0, -49.5, -67.0, -102.5],
        [6.0, 0.0, -7.5, -14.25, -18.8, -22.5, -27.5, -32.5, -42.5, -52.5, -72.5, -113.0],
        [6.0, 0.0, -7.5, -14.5, -19.5, -23.5, -29.5, -34.5, -45.5, -56.3, -78.0, -122.5],
    ],
    # h' = 0.1
    [
        [6.0, 0.0, -6.0, -12.5, -17.3, -20.5, -25.5, -30.0, -37.5, -45.5, -62.0, -94.7],
        [6.0, 0.0, -7.0, -13.5, -18.0, -21.6, -27.2, -32.0, -40.5, -49.5, -67.0, -103.7],
        [6.0, 0.0, -7.5, -14.5, -19.0, -22.8, -29.0, -33.3, -42.9, -52.5, -72.0, -112.0],
    ],
    # h' = 1.0
    [
        [6.0, 0.0, -6.0, -12.5, -16.0, -19.3, -24.0, -27.5, -34.2, -40.4, -52.5, -78.8],
        [6.0, 0.0, -7.0, -13.0, -16.8, -20.5, -25.5, -29.5, -36.0, -42.8, -56.2, -84.0],
        [6.0, 0.0, -7.5, -14.0, -17.7, -21.3, -26.3, -30.8, -37.8, -45.5, -60.0, -89.7],
    ],
]
LOG_H_AXIS = [math.log10(h) for h in H_NORM_AXIS]
FLOOR_DB = -15.0


def table8_kf(ka: float) -> float:
    """k_f from Table 8 — numpy.interp edge-clamps exactly like the engine."""
    return float(np.interp(ka, KA_AXIS, KF_VALS))


def table9_delta_l(h_norm: float, alpha: float, r_norm: float) -> float:
    """ΔL(h', α, R') via tensor-product PCHIP, nested R' -> α -> log10(h').

    Inputs are clipped into each axis domain (no extrapolation), matching the
    engine's ``.clamp()`` on R', the ALPHA query, and the log10(h') query.
    """
    r = float(np.clip(r_norm, R_NORM_AXIS[0], R_NORM_AXIS[-1]))
    # Step 1: PCHIP along R' for each of the 9 (h', α) columns.
    grid = [
        [float(PchipInterpolator(R_NORM_AXIS, TABLE9[hi][ai])(r)) for ai in range(3)]
        for hi in range(3)
    ]
    # Step 2: PCHIP along α for each h'.
    al = float(np.clip(alpha, ALPHA_AXIS[0], ALPHA_AXIS[-1]))
    col = [float(PchipInterpolator(ALPHA_AXIS, grid[hi])(al)) for hi in range(3)]
    # Step 3: PCHIP along log10(h').
    hq = float(np.clip(math.log10(h_norm), LOG_H_AXIS[0], LOG_H_AXIS[-1]))
    return float(PchipInterpolator(LOG_H_AXIS, col)(hq))


def forest_delta_l(f_hz: float, cfg: dict, c0: float) -> float:
    """ΔL_s <= 0 dB (Eqs. 288-291), mirroring the engine step-for-step."""
    n_q = 2.0 * cfg["stem_radius"] * cfg["density"]  # Eq. 290
    t = min((cfg["d_m"] * n_q / 1.75) ** 2, 1.0)  # Eq. 289
    ka = 2.0 * math.pi * f_hz * cfg["stem_radius"] / c0
    kf = table8_kf(ka)
    if kf == 0.0 or t == 0.0:
        return 0.0
    h_norm = n_q * cfg["height"]
    r_norm = min(max(n_q * cfg["d_m"], 0.0625), 10.0)  # clamped both sides (A3)
    a_e = table9_delta_l(h_norm, cfg["absorption"], r_norm) + 20.0 * math.log10(8.0 * r_norm)
    return max(1.25 * kf * t * a_e, FLOOR_DB)  # Eq. 291


# Named parameter sets: an all-zero crossing (d_m = 0 ⇒ T = 0), a light
# crossing, a dense crossing that saturates onto the −15 floor at high bands,
# and a mid-range crossing.
CONFIGS = [
    {
        "name": "zero_crossing",
        "d_m": 0.0, "density": 0.5, "stem_radius": 0.1, "absorption": 0.2, "height": 10.0,
    },
    {
        "name": "light_crossing",
        "d_m": 40.0, "density": 0.1, "stem_radius": 0.1, "absorption": 0.2, "height": 10.0,
    },
    {
        "name": "dense_floor",
        "d_m": 200.0, "density": 0.5, "stem_radius": 0.15, "absorption": 0.3, "height": 15.0,
    },
    {
        "name": "mid_range",
        "d_m": 60.0, "density": 0.2, "stem_radius": 0.1, "absorption": 0.2, "height": 10.0,
    },
]

BAND_INDICES = list(range(0, 105, 8))  # 13 bands, 25 Hz … 10 kHz.


def main() -> None:
    axis = fm.freq_axis()
    c0 = common.C0_M_S
    provenance = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()

    out = Path(__file__).resolve().parents[2] / (
        "crates/envi-harness/tests/fixtures/oracle/forest.toml"
    )
    lines = [
        "# generated by tools/nord2000_oracle/gen_forest_fixtures.py — DO NOT EDIT",
        "# Sub-model 10 forest excess attenuation (AV 1106/07 §5.19, Eqs. 288-291, Tables 8/9).",
        "",
        "[meta]",
        'oracle = "AV 1106/07 §5.19 SM10 transcription (Eqs. 288-291 + Tables 8/9, scipy PCHIP)"',
        f'provenance = "gen_forest_fixtures.py sha256:{provenance}"',
        f"c0 = {c0}",
        "# numpy.interp Table 8 + scipy PchipInterpolator Table 9 (nested R'->α->log10(h')).",
        "tol_abs_db = 1e-9",
        "",
    ]
    for cfg in CONFIGS:
        bands, vals = [], []
        for bi in BAND_INDICES:
            bands.append(bi)
            vals.append(forest_delta_l(axis[bi], cfg, c0))
        lines.append("[[case]]")
        lines.append(f'name = "{cfg["name"]}"')
        for key in ("d_m", "density", "stem_radius", "absorption", "height"):
            lines.append(f"{key} = {cfg[key]}")
        lines.append(f"band_index = {bands}")
        lines.append("delta_l_db = [" + ", ".join(f"{v:.10f}" for v in vals) + "]")
        lines.append("")

    out.write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
