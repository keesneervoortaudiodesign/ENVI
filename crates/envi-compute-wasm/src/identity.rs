//! Marshalled-scene tensor identity (HI-01 / D-09).
//!
//! # Why this exists
//! The OPFS chunk store + manifest are keyed by the tensor-identity hash so an
//! identical re-run *reuses* the hash-keyed tensor (D-09). The Phase-11 real-scene
//! path will key off [`envi_compute::identity::tensor_hash`] (blake3 over the drawn
//! GeoJSON features + met + receiver set). Until the drawn polygon is mapped to
//! per-corridor terrain/impedance, `CalcPanel` marshals a flat homogeneous corridor
//! as a [`PrepareSolveReq`]; this module hashes THAT marshalled scene so the key
//! covers EVERY field that determines the tensor — terrain, atmosphere, coherence,
//! weather, sub-source placements + directivity, receiver positions, forest,
//! isolation, `n_sub` — not a lossy scalar tuple. It replaces the ad-hoc 32-bit
//! FNV `hexHash` CalcPanel used to invent (whose birthday collisions would let two
//! distinct setups map to the same `calc/<hash>/` directory and mis-serve reused
//! tensors).
//!
//! # Frozen encoding (mirrors `envi_compute::identity`, own version tag)
//! Same discipline as the GeoJSON identity closure: a version prefix, domain-tag
//! ASCII markers, `u64` little-endian length prefixes on every sequence, and every
//! `f64` as `to_bits().to_le_bytes()` — **never** serialized JSON text, whose float
//! formatting would be an invisible cross-version coupling. The marshalled-corridor
//! input is a distinct identity domain from the GeoJSON scene hash, so it carries
//! its own `envi-marshalled-tensor-hash-v1` prefix. The `tensor_hash` field of the
//! request is EXCLUDED (it is the very value being derived).

use blake3::Hasher;

use envi_compute::interpolate::Resolution;
use envi_compute::scene_dto::{
    ForestParamsDto, GroundSegmentDto, IsolationSpectrumDto, SoundSpeedProfileDto,
    TerrainProfileDto,
};

use crate::dto::{
    AtmosphereDto, CoherenceInputsDto, DirectionalDto, DirectivityBalloonDto, PrepareSolveReq,
    ReceiverPlacementDto, RotationDto, SubSourcePlacementDto,
};

/// Compute the blake3 tensor-identity hash of a marshalled [`PrepareSolveReq`],
/// covering every tensor-affecting field (the request's own `tensor_hash` field is
/// excluded). Returns a 64-character lowercase hex digest — the OPFS/manifest key
/// (D-09), which the sink's `assertHex` path guard accepts.
#[must_use]
pub fn marshalled_tensor_hash(req: &PrepareSolveReq) -> String {
    let mut h = Hasher::new();
    h.update(b"envi-marshalled-tensor-hash-v1");
    put_u32(&mut h, req.n_sub);

    h.update(b"terrain");
    put_terrain(&mut h, &req.terrain);

    h.update(b"atmos");
    put_atmosphere(&mut h, &req.atmosphere);

    h.update(b"coh");
    put_coherence(&mut h, &req.coherence);

    h.update(b"weather");
    put_option(&mut h, req.weather.as_ref(), put_sound_speed);

    h.update(b"subs");
    put_len(&mut h, req.sub_sources.len());
    for s in &req.sub_sources {
        put_sub_source(&mut h, s);
    }

    // Receivers hashed in ascending global-index order so identity is independent
    // of emission order (mirrors identity.rs's id-sorted receiver hashing).
    h.update(b"recv");
    let mut recv: Vec<&ReceiverPlacementDto> = req.receivers.iter().collect();
    recv.sort_by_key(|r| r.global_index);
    put_len(&mut h, recv.len());
    for r in recv {
        put_u32(&mut h, r.global_index);
        put_f64_3(&mut h, &r.position);
    }

    h.update(b"forest");
    put_option(&mut h, req.forest.as_ref(), put_forest);
    h.update(b"forestlen");
    put_option_f64(&mut h, req.forest_path_length_m);

    h.update(b"iso");
    put_option(&mut h, req.isolation.as_ref(), put_isolation);

    h.finalize().to_hex().to_string()
}

// --- field encoders --------------------------------------------------------

fn put_terrain(h: &mut Hasher, t: &TerrainProfileDto) {
    put_len(h, t.points.len());
    for p in &t.points {
        put_f64(h, p[0]);
        put_f64(h, p[1]);
    }
    put_len(h, t.segments.len());
    for s in &t.segments {
        put_ground_segment(h, s);
    }
}

fn put_ground_segment(h: &mut Hasher, s: &GroundSegmentDto) {
    put_f64(h, s.flow_resistivity);
    put_f64(h, s.roughness);
}

fn put_atmosphere(h: &mut Hasher, a: &AtmosphereDto) {
    put_f64(h, a.temperature_c);
    put_f64(h, a.humidity_pct);
    put_f64(h, a.pressure_kpa);
}

fn put_coherence(h: &mut Hasher, c: &CoherenceInputsDto) {
    for v in [
        c.cv2,
        c.ct2,
        c.t_air_c,
        c.c0,
        c.roughness_r,
        c.f_delta_nu,
        c.d_m,
    ] {
        put_f64(h, v);
    }
}

fn put_sound_speed(h: &mut Hasher, w: &SoundSpeedProfileDto) {
    for v in [w.a, w.b, w.c, w.s_a, w.s_b, w.z0] {
        put_f64(h, v);
    }
}

fn put_sub_source(h: &mut Hasher, s: &SubSourcePlacementDto) {
    put_f64_3(h, &s.position);
    put_option(h, s.directivity.as_ref(), put_directional);
}

fn put_directional(h: &mut Hasher, d: &DirectionalDto) {
    put_balloon(h, &d.balloon);
    put_rotation(h, &d.orientation);
}

fn put_balloon(h: &mut Hasher, b: &DirectivityBalloonDto) {
    put_len(h, b.azimuths_deg.len());
    for v in &b.azimuths_deg {
        put_f64(h, *v);
    }
    put_len(h, b.polars_deg.len());
    for v in &b.polars_deg {
        put_f64(h, *v);
    }
    put_len(h, b.grid_db.len());
    for v in &b.grid_db {
        put_f64(h, *v);
    }
    put_option(h, b.phase_grid_rad.as_ref(), |h, ph| {
        put_len(h, ph.len());
        for v in ph {
            put_f64(h, *v);
        }
    });
}

fn put_rotation(h: &mut Hasher, r: &RotationDto) {
    for row in &r.matrix {
        for v in row {
            put_f64(h, *v);
        }
    }
}

fn put_forest(h: &mut Hasher, f: &ForestParamsDto) {
    put_f64(h, f.density_per_m2);
    put_f64(h, f.stem_radius_m);
    put_f64(h, f.height_m);
    put_option_f64(h, f.absorption);
}

fn put_isolation(h: &mut Hasher, iso: &IsolationSpectrumDto) {
    // Resolution is a small closed enum; hash a stable discriminant byte + the
    // authored values (its length is resolution-determined, but hashed explicitly).
    let tag: u8 = match iso.authored.resolution {
        Resolution::Octave => 1,
        Resolution::Third => 2,
        Resolution::Twelfth => 3,
    };
    h.update(&[tag]);
    put_len(h, iso.authored.values.len());
    for v in &iso.authored.values {
        put_f64(h, *v);
    }
}

// --- primitives (frozen byte encoding, mirror of identity.rs) --------------

/// Hash an `Option<&T>` with a presence marker so `None` never collides with a
/// present-but-empty value.
fn put_option<T>(h: &mut Hasher, v: Option<&T>, f: impl FnOnce(&mut Hasher, &T)) {
    match v {
        Some(inner) => {
            h.update(&[1u8]);
            f(h, inner);
        }
        None => {
            h.update(&[0u8]);
        }
    }
}

/// Hash an `Option<f64>` with a presence marker.
fn put_option_f64(h: &mut Hasher, v: Option<f64>) {
    match v {
        Some(x) => {
            h.update(&[1u8]);
            put_f64(h, x);
        }
        None => {
            h.update(&[0u8]);
        }
    }
}

/// A `[f64; 3]` position.
fn put_f64_3(h: &mut Hasher, v: &[f64; 3]) {
    for c in v {
        put_f64(h, *c);
    }
}

/// `u64` little-endian length prefix.
fn put_len(h: &mut Hasher, n: usize) {
    h.update(&(n as u64).to_le_bytes());
}

/// A `u32` little-endian.
fn put_u32(h: &mut Hasher, v: u32) {
    h.update(&v.to_le_bytes());
}

/// One `f64` as its raw bits, little-endian (bit-exact, serializer-free).
fn put_f64(h: &mut Hasher, v: f64) {
    h.update(&v.to_bits().to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::AtmosphereDto;
    use envi_compute::scene_dto::{GroundSegmentDto, TerrainProfileDto};

    fn base_req() -> PrepareSolveReq {
        PrepareSolveReq {
            tensor_hash: String::new(),
            n_sub: 2,
            terrain: TerrainProfileDto {
                points: vec![[2.5, 0.0], [400.0, 0.0]],
                segments: vec![GroundSegmentDto {
                    flow_resistivity: 200.0,
                    roughness: 0.0,
                }],
            },
            atmosphere: AtmosphereDto {
                temperature_c: 15.0,
                humidity_pct: 70.0,
                pressure_kpa: 101.325,
            },
            coherence: CoherenceInputsDto {
                cv2: 0.0,
                ct2: 0.0,
                t_air_c: 15.0,
                c0: 340.348,
                roughness_r: 0.0,
                f_delta_nu: 1.0,
                d_m: 97.5,
            },
            weather: None,
            sub_sources: vec![SubSourcePlacementDto {
                position: [2.5, 0.0, 0.5],
                directivity: None,
            }],
            receivers: vec![
                ReceiverPlacementDto {
                    global_index: 0,
                    position: [100.0, 0.0, 1.5],
                },
                ReceiverPlacementDto {
                    global_index: 1,
                    position: [101.0, 0.0, 1.5],
                },
            ],
            forest: None,
            forest_path_length_m: None,
            isolation: None,
        }
    }

    #[test]
    fn hash_is_64_char_lowercase_hex_and_deterministic() {
        let req = base_req();
        let a = marshalled_tensor_hash(&req);
        assert_eq!(a.len(), 64, "blake3 hex is 64 chars");
        assert!(
            a.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "lowercase hex only (accepted by the OPFS assertHex guard)"
        );
        assert_eq!(a, marshalled_tensor_hash(&req), "deterministic");
    }

    #[test]
    fn tensor_hash_field_is_excluded_from_identity() {
        // The digest must NOT change when only the (derived) tensor_hash field changes.
        let mut req = base_req();
        let a = marshalled_tensor_hash(&req);
        req.tensor_hash = "placeholder".to_string();
        assert_eq!(
            a,
            marshalled_tensor_hash(&req),
            "tensor_hash field is excluded"
        );
    }

    #[test]
    fn receiver_order_does_not_change_identity() {
        let mut req = base_req();
        let a = marshalled_tensor_hash(&req);
        req.receivers.reverse();
        assert_eq!(
            a,
            marshalled_tensor_hash(&req),
            "receiver order is irrelevant"
        );
    }

    #[test]
    fn every_tensor_affecting_field_changes_identity() {
        let base = marshalled_tensor_hash(&base_req());

        // Receiver position moved 1 mm.
        let mut r = base_req();
        r.receivers[0].position[0] += 0.001;
        assert_ne!(base, marshalled_tensor_hash(&r), "receiver move");

        // Sub-source position moved.
        let mut r = base_req();
        r.sub_sources[0].position[2] += 0.001;
        assert_ne!(base, marshalled_tensor_hash(&r), "sub-source move");

        // Terrain flow resistivity changed.
        let mut r = base_req();
        r.terrain.segments[0].flow_resistivity = 250.0;
        assert_ne!(base, marshalled_tensor_hash(&r), "terrain change");

        // Atmosphere temperature changed.
        let mut r = base_req();
        r.atmosphere.temperature_c = 25.0;
        assert_ne!(base, marshalled_tensor_hash(&r), "atmosphere change");

        // Coherence turbulence changed.
        let mut r = base_req();
        r.coherence.cv2 = 1.0;
        assert_ne!(base, marshalled_tensor_hash(&r), "coherence change");

        // n_sub changed (adding a receiverless sub-source still changes the tensor rows).
        let mut r = base_req();
        r.n_sub = 3;
        r.sub_sources.push(SubSourcePlacementDto {
            position: [2.5, 0.0, 0.8],
            directivity: None,
        });
        assert_ne!(base, marshalled_tensor_hash(&r), "n_sub change");
    }

    #[test]
    fn colliding_scalar_tuple_scenes_get_distinct_hashes() {
        // The old FNV key hashed only (spacing, round(area), n_sub, total). Two
        // scenes with the SAME such tuple but different receiver geometry must now
        // hash differently (the failure HI-01 flags).
        let a = base_req();
        let mut b = base_req(); // same n_sub, same receiver count
        b.receivers[1].position = [200.0, 5.0, 1.5]; // different geometry, same count
        assert_ne!(
            marshalled_tensor_hash(&a),
            marshalled_tensor_hash(&b),
            "same (n_sub, receiver_count) but different geometry must not collide"
        );
    }
}
