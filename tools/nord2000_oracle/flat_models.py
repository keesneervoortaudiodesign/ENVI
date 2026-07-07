"""Independent oracle for the Nord2000 flat-terrain ground effect (Sub-models 1
and 2, AV 1106/07 Eqs. 115-133) plus the Fresnel-zone machinery (Eqs. 338-353)
and PhaseDiffFreq (Eqs. 378-381).

Cross-implementation reference for the ENVI Rust engine: this obtains the
Faddeeva function w(z) from ``scipy.special.wofz`` (independent of the engine's
own three-branch approximation, Eqs. 61-74) and otherwise transcribes the same
equations. It emits the same two-channel, phase-preserving result the engine
produces (h_coh complex + p_incoh real), so the ΔL_t curves cross-check the
engine's Sub-model 1/2 composition at the 0.1 dB level.

Equations are cited by AV 1106/07 report + equation number only (licensing rule,
CLAUDE.md). Developer tool; NOT a build dependency.

Nord2000-native convention: time e^{-jwt}, outgoing phase e^{+jwt}, Im Z_G > 0.
"""

from __future__ import annotations

import math

import common

# The exact 1/12-octave grid mirrors envi-engine::freq (G = 10^0.3).
G = 1.9952623149688795


def freq_axis() -> list[float]:
    """105 exact 1/12-octave centres: 1000 * G^((i-64)/12), i = 0..104."""
    return [1000.0 * G ** ((i - 64) / 12.0) for i in range(105)]


def third_octave_grid() -> list[float]:
    """27 exact 1/3-octave centres 25 Hz..10 kHz (every 4th 1/12-octave point)."""
    axis = freq_axis()
    return [axis[k * 4] for k in range(27)]


# --------------------------------------------------------------------------- #
# Rays (straight, homogeneous) — cancellation-free ΔR (AV 1106/07 §5.5.6).
# --------------------------------------------------------------------------- #
def straight_rays(d: float, h_s: float, h_r: float, c0: float) -> dict:
    r1 = math.hypot(d, h_r - h_s)
    r2 = math.hypot(d, h_r + h_s)
    dr = 4.0 * h_s * h_r / (r1 + r2)  # (R2^2 - R1^2)/(R2+R1)
    psi_g = math.atan2(h_s + h_r, d)
    return {"r1": r1, "r2": r2, "dr": dr, "dtau": dr / c0, "tau2": r2 / c0, "psi_g": psi_g}


# --------------------------------------------------------------------------- #
# Incoherent reflection coefficient rho_i (AV 1106/07 Eqs. 75-76).
# --------------------------------------------------------------------------- #
def incoherent_rho(z_g: complex) -> float:
    x, y = z_g.real, z_g.imag
    m = x * x + y * y
    alpha_ri = (8.0 * x / m) * (
        1.0
        - (x / m) * math.log(1.0 + 2.0 * x + m)
        + ((x * x - y * y) / (y * m)) * math.atan(y / (1.0 + x))
    )
    return math.sqrt(max(0.0, min(1.0, 1.0 - alpha_ri)))


# --------------------------------------------------------------------------- #
# Coherence coefficient F (AV 1106/07 Eqs. 110-114). Homogeneous: F = Ff * Fc
# (FΔν = Fs = 1; Fr = 1 for roughness r = 0).
# --------------------------------------------------------------------------- #
def exp_clamped(x: float) -> float:  # Eq. 337
    if x >= -1.0:
        return math.exp(x)
    if x > -2.0:
        return math.exp(-1.0) * (x + 2.0) ** 2
    return 0.0


def coherence_ff(f_hz: float, dtau: float) -> float:  # Eq. 111
    x = abs(0.23 * math.pi * f_hz * dtau)
    if x <= 1e-15:
        return 1.0
    if x <= math.pi:
        return math.sin(x) / x
    return 0.0


def coherence_f(
    f_hz: float, dtau: float, rho_sep: float, cv2: float, ct2: float, t_c: float, c0: float
) -> float:
    ff = coherence_ff(f_hz, dtau)
    if cv2 == 0.0 and ct2 == 0.0:
        fc = 1.0
    else:
        t_abs = 273.15 + t_c
        turb = ct2 / (t_abs * t_abs) + (22.0 / 3.0) * cv2 / (c0 * c0)
        x = 5.888e-3 * turb * f_hz * f_hz * abs(rho_sep) ** (5.0 / 3.0) * DIST_D
        fc = exp_clamped(-x)
    return ff * fc  # FΔν = Fr = Fs = 1


# Propagation distance for the Fc turbulence integral (set per profile).
DIST_D = 97.5


# --------------------------------------------------------------------------- #
# Sub-model 1 — two-channel flat ground effect (AV 1106/07 Eq. 120).
# --------------------------------------------------------------------------- #
def submodel1(
    f_hz: float,
    rays: dict,
    sigma: float,
    c0: float,
    cv2: float = 0.0,
    ct2: float = 0.0,
    t_c: float = 15.0,
) -> tuple[complex, float]:
    """Return (h_coh_factor, p_incoh)."""
    z_g = common.ground_impedance(f_hz, sigma)
    q = common.spherical_q(f_hz, rays["tau2"], rays["psi_g"], z_g)
    ratio = rays["r1"] / rays["r2"]
    # Transversal separation rho = 2 hS hR/(hS+hR) (Eq. 119); heights per profile.
    h_s, h_r = HEIGHTS
    rho_sep = 2.0 * h_s * h_r / (h_s + h_r) if (h_s + h_r) > 0 else 0.0
    f_coh = coherence_f(f_hz, rays["dtau"], rho_sep, cv2, ct2, t_c, c0)
    phase = complex(math.cos(2 * math.pi * f_hz * rays["dtau"]),
                    math.sin(2 * math.pi * f_hz * rays["dtau"]))  # e^{+j2pi f dtau}
    h_coh = 1.0 + f_coh * ratio * phase * q
    rho_i = incoherent_rho(z_g)
    p_incoh = (1.0 - f_coh * f_coh) * (ratio * rho_i) ** 2
    return h_coh, p_incoh


# Source/receiver heights for rho_sep (set per profile).
HEIGHTS = (0.5, 1.5)


def delta_l_db(h_coh: complex, p_incoh: float) -> float:
    return 10.0 * math.log10(abs(h_coh) ** 2 + p_incoh)  # Eq. 120


# --------------------------------------------------------------------------- #
# Fresnel-zone machinery (AV 1106/07 Eqs. 338-353).
# --------------------------------------------------------------------------- #
def calc_fz_d(r_s: float, r_r: float, theta: float, flp: float) -> float:  # Eq. 339
    r = r_s + r_r
    l = r + flp
    ct = math.cos(theta)
    a = 4.0 * (l * l - (r * ct) ** 2)
    b = 4.0 * r * ct * (r_r * r_r - r_s * r_s) + 4.0 * (r_s - r_r) * l * l * ct
    c = -(l ** 4) + 2.0 * (r_s * r_s + r_r * r_r) * l * l - (r_s * r_s - r_r * r_r) ** 2
    return (-b + math.sqrt(b * b - 4.0 * a * c)) / (2.0 * a)


def fresnel_zone_size(d: float, h_s: float, h_r: float, flp: float) -> tuple[float, float]:
    hsum = h_s + h_r
    psi = math.atan(hsum / d)
    r = math.hypot(hsum, d)
    r_s = h_s / hsum * r
    r_r = h_r / hsum * r
    a1 = calc_fz_d(r_s, r_r, math.pi - psi, flp)  # Eq. 341
    a2 = calc_fz_d(r_s, r_r, psi, flp)  # Eq. 342
    return a1, a2


def _borders(d: float, h_s: float, h_r: float, flp: float) -> tuple[float, float, float]:
    h_s = max(h_s, 0.01)
    h_r = max(h_r, 0.01)
    d_refl = d * h_s / (h_s + h_r)
    a1, a2 = fresnel_zone_size(d, h_s, h_r, flp)
    return d_refl, d_refl - a1, d_refl + a2  # Eq. 350


def _overlap(lo: float, hi: float, a: float, b: float) -> float:
    width = b - a
    if width <= 0.0:
        return 0.0
    return max(0.0, min(hi, b) - max(lo, a)) / width


def fresnel_zone_w(d, h_s, h_r, d1, d2, flp) -> float:  # Eq. 351
    _, d1f, d2f = _borders(d, h_s, h_r, flp)
    return _overlap(d1, d2, d1f, d2f)


def fresnel_zone_wm(d, h_s, h_r, d1, d2, flp) -> float:  # Eq. 353
    d_refl, d1f, d2f = _borders(d, h_s, h_r, flp)
    return 0.5 * (_overlap(d1, d2, d1f, d_refl) + _overlap(d1, d2, d_refl, d2f))


# --------------------------------------------------------------------------- #
# PhaseDiffFreq (AV 1106/07 Eqs. 378-381).
# --------------------------------------------------------------------------- #
def phase_diff_freq(d, h_s, h_r, sigma_min, c0, target) -> float:
    r2 = math.hypot(d, h_s + h_r)
    r1 = math.hypot(d, h_s - h_r)
    dr = r2 - r1
    psi_g = math.asin((h_s + h_r) / r2)

    def psi_of(f: float) -> float:
        arg = common.gamma_p(psi_g, common.ground_impedance(f, sigma_min))
        return 2 * math.pi * f * dr / c0 + math.atan2(arg.imag, arg.real)

    grid = third_octave_grid()
    f_lo, f_hi = grid[0], grid[-1]
    psi_lo, psi_hi = psi_of(f_lo), psi_of(f_hi)
    if target <= psi_lo:
        return max(1e-6, f_lo * target / psi_lo) if abs(psi_lo) > 1e-30 else f_lo
    if target >= psi_hi:
        f_8k = grid[-3]
        psi_8k = psi_of(f_8k)
        slope = (f_hi - f_8k) / max(1e-30, psi_hi - psi_8k)
        return min(100_000.0, max(f_hi, f_hi + (target - psi_hi) * slope))
    for i in range(len(grid) - 1):
        f1, f2 = grid[i], grid[i + 1]
        p1, p2 = psi_of(f1), psi_of(f2)
        if p1 <= target <= p2:
            if abs(p2 - p1) < 1e-30:
                return f1
            log_f = math.log(f1) + (target - p1) / (p2 - p1) * (math.log(f2) - math.log(f1))
            return math.exp(log_f)
    return f_hi


# --------------------------------------------------------------------------- #
# Sub-model 2 — segmented impedance (AV 1106/07 Eqs. 124-133).
# --------------------------------------------------------------------------- #
def _poly(r: float) -> float:  # Eq. 128 S-curve
    return 8.78 * r ** 5 - 21.95 * r ** 4 + 21.76 * r ** 3 - 10.69 * r ** 2 + 3.1 * r


def type_weights(f_hz, strips, d, h_s, h_r, c0) -> list[tuple[tuple, float]]:
    types: list[tuple] = []
    for s in strips:
        key = (s["sigma"], s["rough"])
        if key not in types:
            types.append(key)
    n = len(types)
    flp = 0.25 * c0 / f_hz
    w_low = [0.0] * n
    r_wm = [0.0] * n
    for s in strips:
        i = types.index((s["sigma"], s["rough"]))
        w_low[i] += fresnel_zone_w(d, h_s, h_r, s["x0"], s["x1"], flp)
        r_wm[i] += fresnel_zone_wm(d, h_s, h_r, s["x0"], s["x1"], flp)
    tan_psi = (h_s + h_r) / d
    if tan_psi >= 0.04:
        r_h = 1.0
    elif tan_psi > 0.005:
        r_h = math.log(200.0 * tan_psi) / math.log(8.0)
    else:
        r_h = 0.0
    sigmas: list[float] = []
    for sig, _ in types:
        if sig not in sigmas:
            sigmas.append(sig)

    def rbar_of(sig: float) -> float:
        return sum(r_wm[i] for i, t in enumerate(types) if t[0] == sig)

    sum_rpp = sum(_poly(rbar_of(sig)) for sig in sigmas)
    w_high = [0.0] * n
    for i, (sig, _) in enumerate(types):
        rbar = rbar_of(sig)
        r_prime = (_poly(rbar) / sum_rpp) * (r_wm[i] / rbar) if (sum_rpp > 0 and rbar > 0) else 0.0
        w_high[i] = (r_wm[i] - r_prime) * r_h + r_prime
    sigma_min = min(t[0] for t in types)
    h_mn = max(min(h_s, h_r), 0.01)
    d_alpha_l = math.pi - (1.9483 * math.log(h_mn) + 18.052) * tan_psi
    f_h = phase_diff_freq(d, h_s, h_r, sigma_min, c0, math.pi)
    f_l = phase_diff_freq(d, h_s, h_r, sigma_min, c0, d_alpha_l)
    if f_l > 0.8 * f_h:
        f_l = 0.8 * f_h

    def blend(wl: float, wh: float) -> float:
        if f_hz <= f_l:
            return wl
        if f_hz >= f_h:
            return wh
        t = (math.log(f_h) - math.log(f_hz)) / (math.log(f_h) - math.log(f_l))
        return t * (wl - wh) + wh

    w_prime = [blend(w_low[i], w_high[i]) for i in range(n)]
    total = sum(w_prime)
    if total > 1e-12:
        w_prime = [w / total for w in w_prime]
    return list(zip(types, w_prime))


def submodel2(f_hz, strips, d, h_s, h_r, c0, cv2=0.0, ct2=0.0, t_c=15.0) -> tuple[complex, float]:
    weights = type_weights(f_hz, strips, d, h_s, h_r, c0)
    rays = straight_rays(d, h_s, h_r, c0)
    h_coh = 0.0 + 0.0j
    p_incoh = 0.0
    for (sigma, _rough), w in weights:
        h_i, p_i = submodel1(f_hz, rays, sigma, c0, cv2, ct2, t_c)
        h_coh += w * h_i
        p_incoh += w * w * p_i
    return h_coh, p_incoh
