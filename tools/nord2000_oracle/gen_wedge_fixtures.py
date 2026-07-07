"""Independent Nord2000 wedge-diffraction oracle (Hadden-Pierce, scipy-based).

Cross-implementation reference for the ENVI Rust engine's
``propagation::diffraction`` module. Implements the Hadden-Pierce finite-impedance
wedge solution (AV 1106/07 Eqs. 78-107) independently and pins the results as a
committed TOML fixture (``crates/envi-harness/tests/fixtures/oracle/wedge_il.toml``).

Independence: the spherical-wave reflection coefficient on the wedge faces
(Eq. 80) is imported from ``common.py`` (which obtains the Faddeeva ``w(z)`` from
``scipy.special.wofz`` -- a mature, independent Faddeeva). If the Rust engine's
hand-rolled ``w(z)`` or its four-term wedge sum has a transcription error, this
oracle disagrees.

Equations cited by AV 1106/07 report + number only -- no document text/figures are
reproduced (CLAUDE.md licensing). This is a developer tool that emits committed
fixture DATA; it is NOT a build dependency. The Rust tests run against the TOML
without Python.

Nord2000-native complex convention: time e^{-jwt}, outgoing phase e^{+jwt}.
"""

from __future__ import annotations

import cmath
import hashlib
import math
from pathlib import Path

from common import C0_M_S, spherical_q  # independent Q_hat (scipy wofz)

# --- Fresnel-integral polynomial fits f/g (AV 1106/07 Eqs. 85-86, Tables 4/5) ---
# The SAME method-defined coefficients the engine uses; fidelity to the standard's
# own approximation is required (a "better" Fresnel integral would disagree with
# reference Nord2000 implementations). These are numeric facts, not document text.
F_COEFFS = [
    0.49997531354311, 0.00185249867385, -0.80731059547652, 1.15348730691625,
    -0.89550049255859, 0.44933436012454, -0.15130803310630, 0.03357197760359,
    -0.00447236493671, 0.00023357512010, 0.00002262763737, -0.00000418231569,
    0.00000019048125,
]
G_COEFFS = [
    0.50002414586702, -1.00151717179967, 0.80070190014386, -0.06004025873978,
    -0.50298686904881, 0.55984929401694, -0.33675804584105, 0.13198388204736,
    -0.03513592318103, 0.00631958394266, -0.00073624261723, 0.00005018358067,
    -0.00000151974284,
]
HARD = 1.0e9  # a large real impedance stands in for a hard face (|Z| -> inf)


def _horner(coeffs: list[float], x: float) -> float:
    acc = 0.0
    for c in reversed(coeffs):
        acc = acc * x + c
    return acc


def fres_f(x: float) -> float:
    return 1.0 / (math.pi * x) if x >= 5.0 else _horner(F_COEFFS, x)


def fres_g(x: float) -> float:
    return 1.0 / (math.pi * math.pi * x * x * x) if x >= 5.0 else _horner(G_COEFFS, x)


def _a_d(b: float) -> complex:
    """A_hat_D(B) = Sign(B)*(f(|B|) - j g(|B|))  (Eq. 84)."""
    sign = 1.0 if b >= 0.0 else -1.0
    ab = abs(b)
    return sign * (fres_f(ab) - 1j * fres_g(ab))


def _heaviside(x: float) -> float:
    return 1.0 if x > 0.0 else 0.0  # H(x): 1 for x>0 else 0 (Eq. 354)


def _face_q(n: int, f: float, tau_s: float, tau_r: float,
            theta_s: float, theta_r: float, beta: float,
            z_s: complex, z_r: complex) -> complex:
    """Q_hat_n on the wedge faces (Eq. 80), tau2 = tau_S + tau_R (prescriptive)."""
    tau2 = tau_s + tau_r
    if n == 1:
        return 1.0 + 0.0j
    if n == 2:
        return spherical_q(f, tau2, min(beta - theta_s, math.pi / 2.0), z_s)
    if n == 3:
        return spherical_q(f, tau2, min(theta_r, math.pi / 2.0), z_r)
    # n == 4
    q2 = spherical_q(f, tau2, min(beta - theta_s, math.pi / 2.0), z_s)
    q3 = spherical_q(f, tau2, min(theta_r, math.pi / 2.0), z_r)
    return q2 * q3


def _modify_angles(theta_s: float, theta_r: float, beta: float
                   ) -> tuple[float, float, float]:
    """Angle-modification scheme (AV 1106/07 p. 43) for image points that land
    inside the wedge (ground-reflected / upward-refracted paths)."""
    two_pi = 2.0 * math.pi
    if 0.0 > theta_r > beta - two_pi:
        theta_s = theta_s - theta_r
        beta = beta - theta_r
        theta_r = 0.0
    elif theta_r <= beta - two_pi:
        theta_s = two_pi - (beta - theta_s)
        beta = two_pi
        theta_r = 0.0
    if beta < theta_s < two_pi:
        beta = theta_s
    if theta_s >= two_pi:
        theta_s = two_pi
        beta = two_pi
    return theta_s, theta_r, beta


def pwedge(f, beta, theta_s, theta_r, tau, tau_s, tau_r, ell, r_s, r_r,
           z_s, z_r, n1_only=False):
    """Hadden-Pierce wedge sound pressure (AV 1106/07 Eqs. 78-91).

    ``n1_only`` selects the non-reflecting variant pwedge0 (Eq. 105): only the
    n=1 term, and the face-reflection lit additions (Eqs. 89/90) are dropped.
    """
    theta_s, theta_r, beta = _modify_angles(theta_s, theta_r, beta)
    w = 2.0 * math.pi * f
    nu = math.pi / beta
    thetas = [
        theta_s - theta_r,
        theta_s + theta_r,
        2.0 * beta - (theta_s + theta_r),
        2.0 * beta - (theta_s - theta_r),
    ]
    coef = 2.0 * tau_s * tau_r / (tau * tau) + 0.5  # (2 tS tR / tau^2 + 1/2)
    eps = 1.0e-8  # p. 41 singularity guard: |theta_n - pi| < eps -> subtract eps
    n_terms = 1 if n1_only else 4
    acc = 0.0 + 0.0j
    for n in range(1, n_terms + 1):
        th = thetas[n - 1]
        if abs(th - math.pi) < eps:
            th = th - eps
        a = (nu / 2.0) * (-beta - math.pi + th) + math.pi * _heaviside(math.pi - th)
        abs_a = abs(a)
        cos_a = math.cos(abs_a)
        sinc = 1.0 if abs_a < 1.0e-6 else math.sin(abs_a) / abs_a  # sinc guard
        denom_e = math.sqrt(1.0 + coef * cos_a * cos_a / (nu * nu))
        b = math.sqrt(4.0 * w * tau_s * tau_r / (math.pi * tau)) * cos_a / math.sqrt(
            nu * nu + coef * cos_a * cos_a)
        e_nu = (math.pi / math.sqrt(2.0)) * sinc * cmath.exp(1j * math.pi / 4.0) \
            / denom_e * _a_d(b)
        qn = 1.0 if n1_only else _face_q(
            n, f, tau_s, tau_r, theta_s, theta_r, beta, z_s, z_r)
        acc += qn * a * e_nu
    p = -(1.0 / math.pi) * acc * cmath.exp(1j * w * tau) / ell

    # Lit-zone additions (Eqs. 87-90).
    th1 = theta_s - theta_r
    th2 = theta_s + theta_r
    th3 = 2.0 * beta - (theta_s + theta_r)
    if th1 < math.pi:  # Eq. 88 direct ray
        r1 = math.sqrt(r_s * r_s + r_r * r_r - 2.0 * r_s * r_r * math.cos(th1))
        t1 = math.sqrt(tau_s * tau_s + tau_r * tau_r
                       - 2.0 * tau_s * tau_r * math.cos(th1))
        p += cmath.exp(1j * w * t1) / r1
    if not n1_only:
        if th3 < math.pi:  # Eq. 89 source-face reflection
            r2 = math.sqrt(r_s * r_s + r_r * r_r - 2.0 * r_s * r_r * math.cos(th2))
            t2 = math.sqrt(tau_s * tau_s + tau_r * tau_r
                           - 2.0 * tau_s * tau_r * math.cos(th2))
            psi = abs(math.asin(
                max(-1.0, min(1.0,
                    (r_s * math.sin(theta_s) + r_r * math.sin(theta_r)) / r2))))
            q = spherical_q(f, t2, psi, z_r)
            p += q * cmath.exp(1j * w * t2) / r2
        if th2 < math.pi:  # Eq. 90 receiver-face reflection
            r3 = math.sqrt(r_s * r_s + r_r * r_r - 2.0 * r_s * r_r * math.cos(th3))
            t3 = math.sqrt(tau_s * tau_s + tau_r * tau_r
                           - 2.0 * tau_s * tau_r * math.cos(th3))
            psi = abs(math.asin(
                max(-1.0, min(1.0,
                    (r_s * math.sin(beta - theta_s)
                     + r_r * math.sin(beta - theta_r)) / r3))))
            q = spherical_q(f, t3, psi, z_s)
            p += q * cmath.exp(1j * w * t3) / r3
    return p


def dwedge(f, beta, theta_s, theta_r, tau, tau_s, tau_r, ell, r_s, r_r, z_s, z_r):
    """Diffraction coefficient D_hat = pwedge * ell * e^{-jwt} (Eqs. 92-94)."""
    w = 2.0 * math.pi * f
    return pwedge(f, beta, theta_s, theta_r, tau, tau_s, tau_r, ell, r_s, r_r,
                  z_s, z_r) * ell * cmath.exp(-1j * w * tau)


def p2wedge(f, g, primary, z1s, z1r, z2s, z2r):
    """Two separate wedges (Eqs. 95-99). ``primary`` in {"first","second"}."""
    tau = g["tau_s"] + g["tau_m"] + g["tau_r"]
    ell = g["r_s"] + g["r_m"] + g["r_r"]
    w = 2.0 * math.pi * f
    if primary == "first":  # Eq. 97
        d1 = dwedge(f, g["beta1"], g["theta_1s"], g["theta_1r"],
                    tau, g["tau_s"], g["tau_m"] + g["tau_r"],
                    ell, g["r_s"], g["r_m"] + g["r_r"], z1s, z1r)
        d2 = dwedge(f, g["beta2"], g["theta_2s"], g["theta_2r"],
                    g["tau_m"] + g["tau_r"], g["tau_m"], g["tau_r"],
                    g["r_m"] + g["r_r"], g["r_m"], g["r_r"], z2s, z2r)
    else:  # Eq. 98
        d1 = dwedge(f, g["beta1"], g["theta_1s"], g["theta_1r"],
                    g["tau_s"] + g["tau_m"], g["tau_s"], g["tau_m"],
                    g["r_s"] + g["r_m"], g["r_s"], g["r_m"], z1s, z1r)
        d2 = dwedge(f, g["beta2"], g["theta_2s"], g["theta_2r"],
                    tau, g["tau_s"] + g["tau_m"], g["tau_r"],
                    ell, g["r_s"] + g["r_m"], g["r_r"], z2s, z2r)
    return d1 * d2 * cmath.exp(1j * w * tau) / (ell * ell)


def p2edge(f, g, primary, z1s, z2r):
    """Thick screen, two edges (Eqs. 100-104): hard top faces, factor 0.5."""
    tau = g["tau_s"] + g["tau_m"] + g["tau_r"]
    ell = g["r_s"] + g["r_m"] + g["r_r"]
    w = 2.0 * math.pi * f
    if primary == "first":  # Eq. 102
        d1 = dwedge(f, g["beta1"], g["theta_1s"], g["theta_1r"],
                    tau, g["tau_s"], g["tau_m"] + g["tau_r"],
                    ell, g["r_s"], g["r_m"] + g["r_r"], z1s, HARD)
        d2 = dwedge(f, g["beta2"], g["theta_2s"], g["theta_2r"],
                    g["tau_m"] + g["tau_r"], g["tau_m"], g["tau_r"],
                    g["r_m"] + g["r_r"], g["r_m"], g["r_r"], HARD, z2r)
    else:  # Eq. 103
        d1 = dwedge(f, g["beta1"], g["theta_1s"], g["theta_1r"],
                    g["tau_s"] + g["tau_m"], g["tau_s"], g["tau_m"],
                    g["r_s"] + g["r_m"], g["r_s"], g["r_m"], z1s, HARD)
        d2 = dwedge(f, g["beta2"], g["theta_2s"], g["theta_2r"],
                    tau, g["tau_s"] + g["tau_m"], g["tau_r"],
                    ell, g["r_s"] + g["r_m"], g["r_r"], HARD, z2r)
    return 0.5 * d1 * d2 * cmath.exp(1j * w * tau) / (ell * ell)


# --- Geometry helper: wedge angles for a thin screen (edge above ground) ------

def thin_screen_wedge(s, t, r, beta_frac=0.9999):
    """Build single-wedge inputs from S, T (edge), R for a thin vertical screen.

    theta measured CCW from the receiver-side face (pointing straight down).
    """
    beta = 2.0 * math.pi * beta_frac
    r_s = math.hypot(t[0] - s[0], t[1] - s[1])
    r_r = math.hypot(r[0] - t[0], r[1] - t[1])
    ell = r_s + r_r
    tau_s = r_s / C0_M_S
    tau_r = r_r / C0_M_S
    face = -math.pi / 2.0  # receiver face points down

    def ang(pt):
        phi = math.atan2(pt[1] - t[1], pt[0] - t[0])
        return (phi - face) % (2.0 * math.pi)

    return {
        "beta": beta, "theta_s": ang(s), "theta_r": ang(r),
        "tau_s": tau_s, "tau_r": tau_r, "tau": tau_s + tau_r,
        "r_s": r_s, "r_r": r_r, "ell": ell,
        "r_sr": math.hypot(r[0] - s[0], r[1] - s[1]),
    }


def _fmt(x: float) -> str:
    return repr(float(x))


def main() -> None:
    lines: list[str] = []
    lines.append("# Nord2000 wedge-diffraction oracle fixtures (AV 1106/07 Eqs. 78-107).")
    lines.append("# GENERATED by tools/nord2000_oracle/gen_wedge_fixtures.py -- DO NOT EDIT.")
    lines.append("# Independent Hadden-Pierce impl; wedge-face Q_hat via scipy.special.wofz")
    lines.append("# (imported from common.py). Cross-checks the engine's own w(z) + 4-term sum.")
    lines.append("")
    lines.append("[meta]")
    src = (Path(__file__).parent / "common.py").read_bytes()
    prov = hashlib.sha256(src).hexdigest()[:16]
    lines.append(f'provenance = "common.py sha256:{prov}"')
    lines.append("il_tol_db = 0.05")
    lines.append("q_tol_rel = 1.0e-3")
    lines.append("shadow_tol = 0.01")
    lines.append(f"c0_m_s = {_fmt(C0_M_S)}")
    lines.append("")

    # 1. Hard thin-screen IL grid (the research anchor geometry x 6 frequencies).
    s, t, r = (0.0, 1.0), (50.0, 6.0), (100.0, 1.0)
    g = thin_screen_wedge(s, t, r)
    lines.append("# Hard thin screen S=(0,1) T=(50,6) R=(100,1), beta=2pi*0.9999.")
    lines.append("# il_db = -20 log10(|pwedge| * R_SR): the insertion loss vs free field.")
    for f in (125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0):
        p = pwedge(f, g["beta"], g["theta_s"], g["theta_r"], g["tau"],
                   g["tau_s"], g["tau_r"], g["ell"], g["r_s"], g["r_r"], HARD, HARD)
        il = -20.0 * math.log10(abs(p) * g["r_sr"])
        lines.append("[[il_hard]]")
        lines.append(f"f = {_fmt(f)}")
        lines.append(f"il_db = {_fmt(il)}")
        lines.append(f"p_re = {_fmt(p.real)}")
        lines.append(f"p_im = {_fmt(p.imag)}")
        lines.append("")

    # 2. Shadow-boundary approach series (|p|*ell -> 0.5 from both sides).
    lines.append("# Shadow-boundary series: |p|*ell -> 0.500 as |theta_s-theta_r-pi|->0.")
    r_s = r_r = 50.2494
    ell = r_s + r_r
    tau_s = r_s / C0_M_S
    tau_r = r_r / C0_M_S
    theta_r = math.radians(84.28940686)
    beta = 2.0 * math.pi * 0.9999
    for ddeg in (0.5, 0.1, 0.05, 0.01, -0.01, -0.05, -0.1, -0.5):
        th1 = math.pi + math.radians(ddeg)
        theta_s = theta_r + th1
        p = pwedge(1000.0, beta, theta_s, theta_r, tau_s + tau_r, tau_s, tau_r,
                   ell, r_s, r_r, HARD, HARD)
        lines.append("[[shadow]]")
        lines.append(f"delta_deg = {_fmt(ddeg)}")
        lines.append(f"theta_s = {_fmt(theta_s)}")
        lines.append(f"theta_r = {_fmt(theta_r)}")
        lines.append(f"beta = {_fmt(beta)}")
        lines.append(f"tau_s = {_fmt(tau_s)}")
        lines.append(f"tau_r = {_fmt(tau_r)}")
        lines.append(f"r_s = {_fmt(r_s)}")
        lines.append(f"r_r = {_fmt(r_r)}")
        lines.append(f"mag_ell = {_fmt(abs(p) * ell)}")
        lines.append("")

    # 3. Finite-impedance-face wedge (faces sigma=200 -> Z via Delany-Bazley).
    from common import ground_impedance
    lines.append("# Finite-impedance faces (sigma=200 kPa) on the anchor geometry.")
    g = thin_screen_wedge(s, t, r)
    for f in (250.0, 1000.0, 4000.0):
        z_face = ground_impedance(f, 200.0)
        p = pwedge(f, g["beta"], g["theta_s"], g["theta_r"], g["tau"],
                   g["tau_s"], g["tau_r"], g["ell"], g["r_s"], g["r_r"],
                   z_face, z_face)
        lines.append("[[soft_face]]")
        lines.append(f"f = {_fmt(f)}")
        lines.append("sigma_kpa = 200.0")
        lines.append(f"z_re = {_fmt(z_face.real)}")
        lines.append(f"z_im = {_fmt(z_face.imag)}")
        lines.append(f"theta_s = {_fmt(g['theta_s'])}")
        lines.append(f"theta_r = {_fmt(g['theta_r'])}")
        lines.append(f"beta = {_fmt(g['beta'])}")
        lines.append(f"tau_s = {_fmt(g['tau_s'])}")
        lines.append(f"tau_r = {_fmt(g['tau_r'])}")
        lines.append(f"r_s = {_fmt(g['r_s'])}")
        lines.append(f"r_r = {_fmt(g['r_r'])}")
        lines.append(f"p_re = {_fmt(p.real)}")
        lines.append(f"p_im = {_fmt(p.imag)}")
        lines.append("")

    # 4. Thick screen (p2edge): flat-top trapezoid, edges T1=(15,4) T2=(30,4).
    tw = _two_wedge_from_points((0.0, 1.0), (15.0, 4.0), (30.0, 4.0), (60.0, 1.0))
    lines.append("# Thick screen p2edge: S=(0,1) T1=(15,4) T2=(30,4) R=(60,1), primary=first.")
    for f in (250.0, 1000.0, 4000.0):
        p = p2edge(f, tw, "first", HARD, HARD)
        lines.append("[[p2edge]]")
        lines.append(f"f = {_fmt(f)}")
        _emit_two_wedge(lines, tw)
        lines.append(f"p_re = {_fmt(p.real)}")
        lines.append(f"p_im = {_fmt(p.imag)}")
        lines.append("")

    # 5. Two separated wedges (p2wedge): two thin screens.
    tw2 = _two_wedge_from_points((0.0, 1.0), (25.0, 5.0), (55.0, 5.0), (80.0, 1.0))
    lines.append("# Two wedges p2wedge: S=(0,1) T1=(25,5) T2=(55,5) R=(80,1), primary=first.")
    for f in (250.0, 1000.0, 4000.0):
        p = p2wedge(f, tw2, "first", HARD, HARD, HARD, HARD)
        lines.append("[[p2wedge]]")
        lines.append(f"f = {_fmt(f)}")
        _emit_two_wedge(lines, tw2)
        lines.append(f"p_re = {_fmt(p.real)}")
        lines.append(f"p_im = {_fmt(p.imag)}")
        lines.append("")

    out = Path(__file__).resolve().parents[2] \
        / "crates/envi-harness/tests/fixtures/oracle/wedge_il.toml"
    out.write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote {out}")


def _two_wedge_from_points(s, t1, t2, r):
    """Build a TwoWedgeGeometry from S, T1, T2, R (edges above the SR line).

    Angles per the Fig. 10/11 convention: each wedge measured CCW from its
    receiver-side face (straight down for near-vertical thin/thick screens). The
    two wedge faces of a thick screen share the top segment T1T2.
    """
    face = -math.pi / 2.0

    def ang(edge, pt):
        return (math.atan2(pt[1] - edge[1], pt[0] - edge[0]) - face) % (2.0 * math.pi)

    r_s = math.hypot(t1[0] - s[0], t1[1] - s[1])
    r_m = math.hypot(t2[0] - t1[0], t2[1] - t1[1])
    r_r = math.hypot(r[0] - t2[0], r[1] - t2[1])
    beta = 2.0 * math.pi * 0.9999
    return {
        "beta1": beta, "theta_1s": ang(t1, s), "theta_1r": ang(t1, t2),
        "beta2": beta, "theta_2s": ang(t2, t1), "theta_2r": ang(t2, r),
        "tau_s": r_s / C0_M_S, "tau_m": r_m / C0_M_S, "tau_r": r_r / C0_M_S,
        "r_s": r_s, "r_m": r_m, "r_r": r_r,
    }


def _emit_two_wedge(lines, g):
    for k in ("beta1", "theta_1s", "theta_1r", "beta2", "theta_2s", "theta_2r",
              "tau_s", "tau_m", "tau_r", "r_s", "r_m", "r_r"):
        lines.append(f"{k} = {_fmt(g[k])}")


if __name__ == "__main__":
    main()
