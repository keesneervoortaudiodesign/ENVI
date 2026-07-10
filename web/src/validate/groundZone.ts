// groundZone.ts — the draw-time ground-zone topology classifier (D-07, Surface B). A candidate
// ground_zone polygon is checked against the EXISTING ground zones with turf boolean predicates and
// classified into one of three outcomes so the store can hard-reject a partial cross before it commits.
//
// # Module I/O
// - Input  a candidate polygon feature (the just-drawn ground_zone geometry) + the list of existing
//   ground_zone features already in the store (each with its stable feature id).
// - Output a `GroundZoneClassification`: `ok` (disjoint from every zone), `contained` (fully inside or
//   fully containing another zone — allowed, innermost wins), or `partial-cross` (interiors partially
//   overlap without containment — REJECTED at draw time, D-07) carrying the id of the FIRST existing zone
//   it crosses so the transient banner can zoom to that (existing) zone. There is no acoustic math here —
//   this is pure geometry (SVC-07 does not forbid client-side topology checks).
// - Valid input range: Polygon geometries (WGS84 rings). A non-polygon candidate classifies as `ok`
//   (nothing to reject); a malformed ring is skipped defensively rather than throwing.

import { booleanContains, booleanOverlap, polygon as turfPolygon } from "@turf/turf";
import type { Feature, Polygon } from "geojson";
import type { GeoJSONStoreFeatures } from "terra-draw";

// The three draw-time outcomes. `contained` is allowed (innermost wins); `partial-cross` is rejected.
export type GroundZoneOutcome = "ok" | "contained" | "partial-cross";

export interface GroundZoneClassification {
  readonly outcome: GroundZoneOutcome;
  // The existing zone the candidate partially crosses (only set for `partial-cross`) — the transient
  // reject banner's "Zoom to conflicting zone" targets THIS existing object, not the rejected candidate.
  readonly conflictId: string | null;
}

// An existing ground zone to test the candidate against: its store feature id + its geometry.
export interface ExistingZone {
  readonly id: string;
  readonly feature: GeoJSONStoreFeatures;
}

// Convert a store feature to a turf Polygon feature, or null if it is not a usable closed polygon. turf's
// `polygon()` throws on a ring with < 4 positions or an unclosed ring — treat that as "not comparable".
function toTurfPolygon(feature: GeoJSONStoreFeatures): Feature<Polygon> | null {
  const geometry = feature.geometry;
  if (!geometry || geometry.type !== "Polygon") {
    return null;
  }
  try {
    return turfPolygon(geometry.coordinates as number[][][]);
  } catch {
    return null; // degenerate / unclosed ring — cannot classify against it, skip defensively
  }
}

// Classify a candidate ground_zone against the existing zones (D-07). A partial cross wins immediately
// (reject); otherwise a containment relationship (either direction) is `contained` (allowed); otherwise
// the candidate is disjoint from every zone → `ok`.
export function classifyGroundZone(
  candidate: GeoJSONStoreFeatures,
  existingZones: readonly ExistingZone[],
): GroundZoneClassification {
  const cand = toTurfPolygon(candidate);
  if (!cand) {
    return { outcome: "ok", conflictId: null }; // non-polygon candidate — nothing to reject
  }

  let contained = false;
  for (const zone of existingZones) {
    const other = toTurfPolygon(zone.feature);
    if (!other) {
      continue;
    }
    // A partial interior overlap with neither polygon containing the other is the reject case (D-07).
    if (booleanOverlap(cand, other)) {
      return { outcome: "partial-cross", conflictId: zone.id };
    }
    // Full containment either direction is allowed (innermost wins) — record it but keep scanning for a
    // partial cross against another zone, which would still have to reject.
    if (booleanContains(other, cand) || booleanContains(cand, other)) {
      contained = true;
    }
  }

  return { outcome: contained ? "contained" : "ok", conflictId: null };
}
