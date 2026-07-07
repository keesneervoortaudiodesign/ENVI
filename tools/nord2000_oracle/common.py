"""Independent Nord2000 ground reflection-coefficient oracle (scipy-based).

This is a *cross-implementation* reference for the ENVI Rust engine: it computes
the same quantities (AV 1106/07 Eqs. 57-60) but obtains the Faddeeva function
w(z) from ``scipy.special.wofz`` (a mature, independent implementation) instead
of the document's own three-branch polynomial approximation (Eqs. 61-74). If the
Rust engine's hand-rolled w(z) has a transcription error, this oracle disagrees.

Equations are cited by AV 1106/07 report + equation number only — no document
text or figures are reproduced (licensing rule, CLAUDE.md). This script is a
developer tool that generates committed fixture DATA; it is NOT a build
dependency. Rust tests run against the committed TOML without needing Python.

Nord2000-native complex convention: time e^{-jwt}, outgoing phase e^{+jwt},
impedance Im > 0.
"""

from __future__ import annotations

import math

from scipy.special import wofz  # independent Faddeeva w(z) = exp(-z^2) erfc(-i z)

# Speed of sound at 15 C (AV 1106/07 Eq. 335: c = 20.05 * sqrt(t + 273.15)).
C0_M_S = 20.05 * math.sqrt(15.0 + 273.15)  # 340.348 m/s


def ground_impedance(f_hz: float, sigma_kpa: float) -> complex:
    """Delany-Bazley normalized ground impedance Z_G (AV 1106/07 Eq. 57).

    sigma in kPa*s*m^-2 (the FORCE .xls / Table 2 unit); the SI-Pa form's
    1000*f/sigma argument equals f/sigma here. Im(Z_G) > 0 (e^{-jwt}).
    """
    x = f_hz / sigma_kpa
    return complex(1.0 + 9.08 * x ** -0.75, 11.9 * x ** -0.73)


def gamma_p(psi_g: float, z_g: complex) -> complex:
    """Plane-wave reflection coefficient (AV 1106/07 Eq. 59)."""
    s = math.sin(psi_g)
    inv_z = 1.0 / z_g
    return (s - inv_z) / (s + inv_z)


def spherical_q(f_hz: float, tau2_s: float, psi_g: float, z_g: complex) -> complex:
    """Spherical-wave reflection coefficient Q (AV 1106/07 Eqs. 58 + 60).

    rho_hat = ((1+j)/2) * sqrt(w*tau2) * (sin psi_g + 1/Z_G),  w = 2 pi f.
    E_hat   = 1 + j sqrt(pi) rho_hat w(rho_hat).
    Q       = Gamma_p + (1 - Gamma_p) E_hat.  No |Q| clamp (surface wave).
    """
    s = math.sin(psi_g)
    inv_z = 1.0 / z_g
    gp = gamma_p(psi_g, z_g)
    omega_tau = 2.0 * math.pi * f_hz * tau2_s
    rho = complex(0.5, 0.5) * math.sqrt(omega_tau) * (s + inv_z)
    e_hat = 1.0 + 1j * math.sqrt(math.pi) * rho * wofz(rho)
    return gp + (1.0 - gp) * e_hat


def faddeeva_w(z: complex) -> complex:
    """Faddeeva w(z) via scipy (the independent reference for Eqs. 61-74)."""
    return complex(wofz(z))


def geometry_tau2(h_s: float, h_r: float, d: float) -> tuple[float, float]:
    """Reflected-path travel time tau2 and grazing angle psi_g (straight ray).

    R2 = sqrt(d^2 + (h_s + h_r)^2); psi_g = atan((h_s + h_r)/d).
    """
    r2 = math.hypot(d, h_s + h_r)
    psi_g = math.atan2(h_s + h_r, d)
    return r2 / C0_M_S, psi_g
