//! Small shared GeoJSON [`Feature`] constructors for the ingestion layers.
//!
//! The three feature producers ([`crate::buildings`], [`crate::landcover`],
//! [`crate::terrain`]) each build a `Feature` with **no bbox**, **no `id`** (TS
//! owns UUIDs via `crypto.randomUUID()`), and a stamped property bag — differing
//! only in geometry. These helpers centralize that literal so the shape and the
//! "no Rust id" invariant live in one place rather than being restated at every
//! call site.

use geojson::{Feature, Geometry, GeometryValue, JsonObject, Position};

/// A polygon [`Feature`] from its GeoJSON `rings` (exterior first, then any
/// holes) and property bag. No bbox, no `id` (TS owns UUIDs).
#[must_use]
pub(crate) fn polygon_feature(rings: Vec<Vec<Position>>, properties: JsonObject) -> Feature {
    Feature {
        bbox: None,
        geometry: Some(Geometry::new(GeometryValue::Polygon { coordinates: rings })),
        id: None,
        properties: Some(properties),
        foreign_members: None,
    }
}

/// A point [`Feature`] from a `[lon, lat]` position and property bag. No bbox,
/// no `id` (TS owns UUIDs).
#[must_use]
pub(crate) fn point_feature(position: impl Into<Position>, properties: JsonObject) -> Feature {
    Feature {
        bbox: None,
        geometry: Some(Geometry::new(GeometryValue::new_point(position))),
        id: None,
        properties: Some(properties),
        foreign_members: None,
    }
}
