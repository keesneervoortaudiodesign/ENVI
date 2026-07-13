// full-journey.spec.ts — THE comprehensive in-browser journey: one continuous offline session that
// drives the whole ENVI application the way a real user does, end to end — GIS import → weather →
// author every object kind → a GENUINE threaded-WASM Nord2000 solve → spectra → isophone map →
// interactive conditioning → scenarios + difference map → exports → the honest stale/409 refusal.
//
// The per-feature specs (import / weather-import / sc1-author-every-kind / calc / results-* /
// isophone / conditioning / scenarios / export / objectStyling) each prove ONE surface in isolation.
// This spec proves they compose: that the same browser session, the same OPFS, the same project and
// the same solved tensor carry a user from an empty map to exported results without a seam falling
// over. A regression that only appears when two surfaces meet (a clobbered manifest, an OPFS key
// collision, a stale isophone feed) is invisible to the focused specs and caught here.
//
// # Module I/O
// - Input  the REAL Vite-served bundle in headless Chromium, with the ENTIRE network intercepted
//   (`bootOffline` guard + basemap + `/api/v1`), then the committed fixtures served for every
//   third-party surface the app really talks to: GIS (`installGisMocks` — PDOK/AHN, the GLO-30 +
//   WorldCover byte proxy, Overpass), Open-Meteo (`installWeatherMocks` — BOTH date-switched
//   products), `/meta` (`installMetaMocks`) and `POST /dgm/triangulate` (`installTriangulateMock`).
//   Scene edits go through the DEV `window.__enviTest` bridge — the same store paths a finished
//   Terra Draw edit takes (headless WebGL pointer-drawing is unreliable; the plan permits this).
// - Output the composed assertions below. Every step asserts an outcome a BROKEN implementation
//   would fail — never merely that a click did not throw.
// - Invariant  ZERO network egress for the whole session: the `unmocked` collector must end empty.
//   No credentials, no live Open-Meteo/basemap connection. (The live counterpart is
//   `live-smoke.spec.ts`, opt-in via ENVI_E2E_LIVE=1 — it is the only spec that touches the wire.)
//
// # The solve is REAL and hard-fails (deliberate)
// The calculation step runs a genuine `wasm-bindgen-rayon` threaded solve over a shared
// `WebAssembly.Memory` (cross-origin-isolated by the COOP/COEP headers) to `done`, and the spectrum
// panel + isophone map are fed by the PRODUCTION `applyResultsFeed` link — no DEV fixture seed.
// Unlike `calc.spec.ts` Test 2 and `results-real-solve.spec.ts`, this spec does NOT skip when the
// pool fails to start: a broken shared-memory build must go RED here, not quietly green. That
// regression has bitten this repo once already (the non-shared `WebAssembly.Memory`, fixed in
// 1099b24), and a skip is exactly how it stayed hidden.
//
// # KNOWN COVERAGE BOUND — read before trusting this spec (do not paper over)
// `compute/marshalScene.ts` currently marshals a FLAT, HOMOGENEOUS corridor: it emits
// `weather: null`, `forest: null`, `isolation: null` and no screen geometry. So the objects authored
// in step 4 and the weather derived in step 3 are REAL and reach the store, the map, the wire and
// the PUT — but they do NOT yet reach the solver. This journey therefore proves
// "author + import + solve + read back", NOT "a drawn wall attenuates the solved level". That
// wiring is the documented open refinement (see ROADMAP Phase 11 / README "Known refinements").
// Step 8 PINS this bound with an explicit assertion: when the terrain/weather marshalling lands,
// that assertion FAILS ON PURPOSE — extend this journey to assert the objects change the result,
// rather than deleting the pin.

import { readFile } from "node:fs/promises";

import { expect, test, type Download, type Page, type Route } from "@playwright/test";

import {
  bootOffline,
  importViewport,
  installGisMocks,
  installMetaMocks,
  installTriangulateMock,
  installWeatherMocks,
} from "./_mocks";

const PROJECT_ID = "5eed0000-0000-4000-8000-00000000f011";
const PROJECT_NAME = "E2E Full Journey";

// The committed import viewport (Dutch AHN coverage) — the site the whole session is anchored on, so
// the imported terrain, the derived weather and the solved calc area all describe the SAME place.
const { viewport } = importViewport();
const SITE = { lng: (viewport.min_lon + viewport.max_lon) / 2, lat: (viewport.min_lat + viewport.max_lat) / 2 };

// The 9 locked scene kinds (SCN-01..04 / WEB-01).
const KINDS = [
  "source",
  "receiver",
  "wall",
  "building",
  "forest",
  "ground_zone",
  "elevation_point",
  "elevation_line",
  "calc_area",
] as const;

// A date inside the Forecast window (ahead of today) and one deep in the Archive/ERA5 window — the two
// date-switched Open-Meteo products the real `fetchWeather` targets by host.
const FORECAST_DATE = new Date(Date.now() + 2 * 24 * 3600 * 1000).toISOString().slice(0, 10);
const ARCHIVE_DATE = "2024-01-15";

// Sample the calc-job lifecycle CONTINUOUSLY across `action`, and return every state observed.
//
// This is how "no re-propagation" is proven honestly. A static reading afterwards is not enough: after a
// real solve the job sits in the TERMINAL `done` state (not `idle` — that is only true of the
// fixture-seeded specs, which never run a job), so re-reading `done` would also be consistent with a
// solve that re-ran and finished. Sampling THROUGH the action means a re-submit cannot hide: it would
// have to pass through `queued`/`running` to get back to `done`, and we would catch it.
async function recordJobStates(page: Page, action: () => Promise<void>): Promise<string[]> {
  await page.evaluate(() => {
    const w = window as unknown as { __jobStates?: string[]; __jobTimer?: number };
    w.__jobStates = [];
    w.__jobTimer = window.setInterval(() => {
      w.__jobStates?.push(window.__enviTest.calcJobState());
    }, 50);
  });
  await action();
  return page.evaluate(() => {
    const w = window as unknown as { __jobStates?: string[]; __jobTimer?: number };
    if (w.__jobTimer !== undefined) window.clearInterval(w.__jobTimer);
    return w.__jobStates ?? [];
  });
}

// Assert the sampled lifecycle never re-entered a solve.
function expectNoResolve(states: string[], what: string): void {
  const solving = states.filter((s) => s === "queued" || s === "running");
  expect(
    solving,
    `${what} re-ran propagation (observed job states: ${[...new Set(states)].join(" → ") || "none"})`,
  ).toEqual([]);
}

async function grabExport(
  page: Page,
  format: "geotiff" | "geojson" | "csv",
): Promise<{ download: Download; bytes: Buffer }> {
  await expect(page.getByTestId("export-open")).toBeEnabled();
  await page.getByTestId("export-open").click();
  await expect(page.getByTestId("export-menu-list")).toBeVisible();
  const downloadPromise = page.waitForEvent("download");
  await page.getByTestId(`export-${format}`).click();
  const download = await downloadPromise;
  const path = await download.path();
  return { download, bytes: await readFile(path) };
}

test("FULL JOURNEY: import → weather → every object → a REAL solve → spectra → map → conditioning → scenarios → export, one offline session", async ({
  page,
}) => {
  // A real threaded solve under software WebGL is the long pole; the rest is cheap.
  test.setTimeout(300_000);

  const unmocked = await bootOffline(page);
  await installMetaMocks(page);
  const gis = await installGisMocks(page);
  const weather = await installWeatherMocks(page);
  await installTriangulateMock(page);

  // Authored in step 4, reused in step 8 as a scene-neutral handle to force one object-layer re-apply.
  let wallId = "";
  // Capture the coalesced whole-scene PUT so the authored scene can be asserted on the wire.
  let putBody: { features: { properties?: { kind?: string } }[] } | null = null;
  await page.route(/\/api\/v1\/projects\/[^/]+\/scene$/, async (route: Route) => {
    if (route.request().method() === "PUT") {
      putBody = route.request().postDataJSON() as typeof putBody;
    }
    await route.fulfill({ status: 200, contentType: "application/json", body: "{}" });
  });

  await test.step("1 — the app boots cross-origin isolated (the solve prerequisite)", async () => {
    await expect(page.getByTestId("object-palette")).toBeVisible();
    await expect(page.getByTestId("map-ready")).toHaveText("yes");
    // Without COOP/COEP there is no SharedArrayBuffer, so the rayon pool could never start. Assert the
    // precondition explicitly — otherwise a header regression shows up as a mystifying solve timeout.
    expect(
      await page.evaluate(() => self.crossOriginIsolated === true),
      "not cross-origin isolated — COOP/COEP headers regressed; the threaded solve cannot run",
    ).toBe(true);
    await page.evaluate(
      ({ id, name }) => window.__enviTest.openProject(id, name),
      { id: PROJECT_ID, name: PROJECT_NAME },
    );
  });

  await test.step("2 — GIS import: terrain + land cover + buildings land as EDITABLE objects on a DGM", async () => {
    await page.evaluate((bbox) => window.__enviTest.runImport(bbox), viewport);

    await expect(page.getByTestId("import-status-terrain")).toHaveText("done", { timeout: 30_000 });
    await expect(page.getByTestId("import-status-landcover")).toHaveText("done", { timeout: 30_000 });
    await expect(page.getByTestId("import-status-buildings")).toHaveText("done", { timeout: 30_000 });

    // Imported data is ordinary editable scene state ("check and complete"), not a locked raster layer.
    const kinds = await page.evaluate(() => Object.values(window.__enviTest.state().kinds));
    expect(kinds).toContain("elevation_point"); // terrain  → editable elevation points
    expect(kinds).toContain("ground_zone"); //     WorldCover → impedance zones
    expect(kinds).toContain("building"); //        Overpass  → footprints

    // The imported elevation set re-triangulates the DGM through the real debounced producer.
    await expect(page.getByTestId("dgm-triangle-count")).toHaveText("1", { timeout: 20_000 });

    // Every contributing open-data source is credited on the map (DATA licence obligation).
    const attribution = page.getByTestId("import-attribution");
    await expect(attribution).toContainText("AHN");
    await expect(attribution).toContainText("ESA WorldCover");
    await expect(attribution).toContainText("OpenStreetMap");

    // The impedance debug overlay renders the effective ground class over the imported zones.
    await page.getByTestId("import-debug-toggle").check();
    expect(await page.evaluate(() => window.__enviTest.importState().debugOverlay)).toBe(true);
    await page.getByTestId("import-debug-toggle").uncheck();

    // Each real GIS source was actually fetched (a stubbed-out import would leave these at 0).
    expect(gis.counts.pdok, "AHN terrain was never fetched").toBeGreaterThan(0);
    expect(gis.counts.proxyWorldcover, "WorldCover was never fetched through the byte proxy").toBeGreaterThan(0);
    expect(gis.counts.overpass, "Overpass buildings were never fetched").toBeGreaterThan(0);
  });

  await test.step("3 — weather: BOTH Open-Meteo products derive per-azimuth A/B/C; a what-if re-derives with ZERO calls", async () => {
    await expect(page.getByTestId("weather-panel")).toBeVisible();
    await expect(page.getByTestId("weather-import")).toBeEnabled({ timeout: 20_000 });

    // (a) Forecast product — a near-future date.
    await page.getByTestId("weather-date").fill(FORECAST_DATE);
    await page.getByTestId("weather-hour").fill("12");
    await page.getByTestId("weather-import").click();
    await expect(page.getByTestId("weather-status")).toHaveText("derived", { timeout: 30_000 });
    expect(weather.counts.forecast, "the Forecast product was never hit").toBeGreaterThan(0);

    // The per-azimuth A/B/C table is the METX-01 payload — downwind/upwind/crosswind all derived.
    await expect(page.getByTestId("weather-abc-table")).toBeVisible();
    for (const az of [0, 90, 180, 270]) {
      await expect(page.getByTestId(`weather-abc-${az}`)).toBeVisible();
    }
    await expect(page.getByTestId("weather-cache")).toHaveText("network fetch");
    await expect(page.getByTestId("weather-callcost")).toContainText("call-cost weight");

    // (b) Archive/ERA5 product — a historical date takes the OTHER host (the date-switch branch).
    await page.getByTestId("weather-date").fill(ARCHIVE_DATE);
    await page.getByTestId("weather-import").click();
    await expect(page.getByTestId("weather-status")).toHaveText("derived", { timeout: 30_000 });
    expect(weather.counts.archive, "a historical date did not hit the Archive/ERA5 product").toBeGreaterThan(0);

    // (c) SC4 zero-egress: a what-if edit (z₀) re-derives from the OPFS cache with NO new API call.
    const callsBefore = weather.counts.forecast + weather.counts.archive;
    await page.getByTestId("weather-z0").fill("0.5");
    await page.getByTestId("weather-import").click();
    await expect(page.getByTestId("weather-status")).toHaveText("derived", { timeout: 30_000 });
    await expect(page.getByTestId("weather-cache")).toHaveText("OPFS cache (no call)");
    expect(
      weather.counts.forecast + weather.counts.archive,
      "a cached what-if edit issued a live Open-Meteo call — the OPFS cache is not being read",
    ).toBe(callsBefore);
  });

  await test.step("4 — author EVERY object kind (+ inheritance, topology reject, isolation spectrum)", async () => {
    // All 9 kinds via the real palette → active-kind → commit path.
    for (let i = 0; i < KINDS.length; i += 1) {
      const kind = KINDS[i];
      await page.getByTestId(`tool-${kind}`).click();
      await expect(page.getByTestId(`tool-${kind}`)).toHaveAttribute("aria-pressed", "true");
      await page.evaluate(
        ([lng, lat]) => window.__enviTest.commitActive(lng as number, lat as number),
        [SITE.lng + i * 0.0004, SITE.lat + i * 0.0004],
      );
    }
    const placed = await page.evaluate(() => Object.values(window.__enviTest.state().kinds));
    for (const kind of KINDS) {
      expect(placed, `store is missing kind ${kind}`).toContain(kind);
    }

    // Last-object inheritance: a second ground_zone seeds from the first until the field is edited.
    await page.evaluate((s) => window.__enviTest.commit("ground_zone", s.lng + 0.01, s.lat + 0.01), SITE);
    await expect(page.getByTestId("inspector-ground_zone")).toBeVisible();
    await page.getByTestId("ground-impedance").selectOption("C");
    await page.getByTestId("ground-roughness").selectOption("M");
    await page.evaluate((s) => window.__enviTest.commit("ground_zone", s.lng + 0.012, s.lat + 0.012), SITE);
    await expect(page.getByTestId("ground-impedance")).toHaveValue("C");
    await expect(page.getByTestId("ground-roughness")).toHaveValue("M");
    await expect(page.getByTestId("inspector")).toContainText("inherited from last ground_zone");

    // Draw-time topology (SCN-02): a clean zone commits, a CONTAINED zone is allowed (innermost wins),
    // and a PARTIALLY-CROSSING zone is hard-rejected at draw time — the engine's segmented-impedance
    // model has no meaning for a partial overlap, so it must never reach the store.
    const base = await page.evaluate((s) => {
      const ring: [number, number][] = [
        [s.lng + 0.02, s.lat + 0.02],
        [s.lng + 0.03, s.lat + 0.02],
        [s.lng + 0.03, s.lat + 0.03],
        [s.lng + 0.02, s.lat + 0.03],
        [s.lng + 0.02, s.lat + 0.02],
      ];
      return window.__enviTest.commitGroundZone(ring);
    }, SITE);
    expect(base.outcome).toBe("ok");

    const contained = await page.evaluate((s) => {
      const ring: [number, number][] = [
        [s.lng + 0.022, s.lat + 0.022],
        [s.lng + 0.028, s.lat + 0.022],
        [s.lng + 0.028, s.lat + 0.028],
        [s.lng + 0.022, s.lat + 0.028],
        [s.lng + 0.022, s.lat + 0.022],
      ];
      return window.__enviTest.commitGroundZone(ring);
    }, SITE);
    expect(contained.outcome, "a fully-contained ground_zone was rejected — containment must be allowed").toBe(
      "contained",
    );
    expect(contained.id).not.toBeNull();

    const crossing = await page.evaluate((s) => {
      const ring: [number, number][] = [
        [s.lng + 0.025, s.lat + 0.025],
        [s.lng + 0.035, s.lat + 0.025],
        [s.lng + 0.035, s.lat + 0.035],
        [s.lng + 0.025, s.lat + 0.035],
        [s.lng + 0.025, s.lat + 0.025],
      ];
      return window.__enviTest.commitGroundZone(ring);
    }, SITE);
    expect(crossing.outcome, "a partially-crossing ground_zone was silently accepted").toBe("partial-cross");
    expect(crossing.id, "a rejected ground_zone still reached the store").toBeNull();
    await expect(page.getByTestId("reject-banner")).toBeVisible();

    // A semi-transparent wall carries an isolation spectrum authored at 1/1-octave: the 9 anchors land on
    // the EXACT octave band indices (4,16,…,100) and the dense 105-grid preview is SERVER-derived (D-05).
    wallId = await page.evaluate(
      (s) => window.__enviTest.commit("wall", s.lng + 0.04, s.lat + 0.04),
      SITE,
    );
    await expect(page.getByTestId("inspector-wall")).toBeVisible();
    await page.getByTestId("wall-edit-spectrum").click();
    await expect(page.getByTestId("spectrum-editor")).toBeVisible();
    await page.getByTestId("spectrum-res-octave").click();
    for (const idx of [4, 16, 28, 40, 52, 64, 76, 88, 100]) {
      await expect(page.getByTestId(`spectrum-anchor-${idx}`)).toHaveCount(1);
    }
    await expect(page.getByTestId("spectrum-anchor-5")).toHaveCount(0); // 5 is NOT an octave centre
    await page.getByTestId("spectrum-cell-64").fill("42");
    await page.getByTestId("spectrum-close").click();
    expect(await page.evaluate((id) => window.__enviTest.spectrum(id), wallId)).not.toBeNull();

    // The authored scene reaches the wire: the coalesced PUT carries every kind.
    await page.getByTestId("save-scene").click();
    await expect.poll(() => putBody, { timeout: 10_000 }).not.toBeNull();
    const payloadKinds = new Set(
      (putBody as unknown as { features: { properties?: { kind?: string } }[] }).features.map(
        (f) => f.properties?.kind,
      ),
    );
    for (const kind of KINDS) {
      expect(payloadKinds, `PUT payload missing kind ${kind}`).toContain(kind);
    }
  });

  await test.step("5 — the map survives a basemap switch with the whole authored scene intact", async () => {
    const before = await page.evaluate(() => window.__enviTest.featureIds().length);
    expect(before).toBeGreaterThan(0);
    // setStyle() destroys every source/layer; the single `style.load` hook must re-hydrate from the
    // canonical store (a lost-on-switch bug drops the scene here).
    await page.getByTestId("switch-basemap").click();
    await expect(page.getByTestId("map-ready")).toHaveText("yes");
    expect(await page.evaluate(() => window.__enviTest.featureIds().length)).toBe(before);
    await page.getByTestId("switch-basemap").click();
    await expect(page.getByTestId("map-ready")).toHaveText("yes");
    expect(await page.evaluate(() => window.__enviTest.featureIds().length)).toBe(before);
  });

  await test.step("6 — calculation: a pre-run cost estimate, then a GENUINE threaded-WASM solve to `done`", async () => {
    await expect(page.getByTestId("calc-panel")).toBeVisible();
    // A coarse fine-spacing keeps the FINE tier small so the real solve finishes in-test.
    await page.getByTestId("calc-spacing").fill("20");
    // The cost estimate is a REAL wasm cost model (receiver count + tensor bytes), shown BEFORE Run.
    await expect(page.getByTestId("calc-estimate")).toBeVisible({ timeout: 30_000 });
    await expect(page.getByTestId("calc-tiers")).toBeVisible();

    await expect(page.getByTestId("calc-run")).toBeEnabled({ timeout: 30_000 });
    await page.getByTestId("calc-run").click();

    // HARD-FAIL (not skip): the pool must start. A non-shared WebAssembly.Memory regression — the exact
    // bug that shipped once — leaves the status pinned at `queued`, and that must be RED.
    await expect(
      page.getByTestId("calc-status"),
      "the threaded-WASM pool never started (shared-memory build regression?) — the solve never ran",
    ).not.toHaveText("queued", { timeout: 45_000 });

    // The genuine Nord2000 solve runs its tiers to completion.
    await expect(page.getByTestId("calc-status")).toHaveText("done", { timeout: 180_000 });
  });

  await test.step("7 — spectra: the REAL solved tensor drives the panel (no fixture seed)", async () => {
    // The PRODUCTION `applyTierComplete → setManifest` feed lit the panel up — the receivers offered here
    // are the ones the solve actually produced.
    await expect(page.getByTestId("results-panel")).toBeVisible();
    const firstReceiver = page.locator('[data-testid^="spectrum-receiver-"]').first();
    await expect(firstReceiver).toBeVisible({ timeout: 30_000 });
    await firstReceiver.click();

    const chart = page.getByTestId("spectrum-chart");
    await expect(chart).toBeVisible({ timeout: 30_000 });

    // 1/3-octave display aggregates the 105-point grid BY BAND INDEX → 27 bands; the expert view shows all 105.
    await expect(chart).toHaveAttribute("data-band-count", "27");
    expect(await page.getByTestId("spectrum-band-bar").count()).toBe(27);
    await page.getByTestId("spectrum-display-twelfth").click();
    expect(await page.getByTestId("spectrum-band-bar").count()).toBe(105);
    await page.getByTestId("spectrum-display-third").click();

    // dB(A) ⇄ dB(C) toggle: both weightings are precomputed at the exact grid centres, so the total moves.
    const total = page.getByTestId("spectrum-total");
    await expect(total).toBeVisible();
    const dba = await total.textContent();
    await page.getByTestId("spectrum-weighting-C").click();
    await expect(page.getByTestId("spectrum-loading")).toHaveCount(0);
    expect(await total.textContent(), "the dB(C) total equals the dB(A) total — the weighting toggle is inert").not.toBe(dba);

    // The two-channel contract is visible to the user: the coherent/incoherent split.
    await page.getByTestId("spectrum-split-toggle").click();
    await expect(page.getByTestId("spectrum-split-overlay")).toBeVisible();
    await expect(page.getByTestId("spectrum-total-coherent")).toBeVisible();
    await expect(page.getByTestId("spectrum-total-incoherent")).toBeVisible();
  });

  await test.step("8 — the isophone map is fed by the SAME real solve, and re-contours with no re-solve", async () => {
    // Fill polygons, never a heatmap raster (WEB-06), traced from the solved fine-tier grid.
    await expect
      .poll(async () => (await page.evaluate(() => window.__enviTest.isophoneTelemetry())).layerType, {
        timeout: 60_000,
      })
      .toBe("fill");
    const t0 = await page.evaluate(() => window.__enviTest.isophoneTelemetry());
    expect(t0.traceCount, "the real solve produced no isophone contours").toBeGreaterThan(0);

    // Editing the colour scale RE-CONTOURS the cached grid — it must never re-run propagation.
    await page.getByTestId("colorscale-preset-viridis").click();
    await expect
      .poll(() => page.evaluate(() => window.__enviTest.colorScaleState().preset))
      .toBe("viridis");

    // The breaks are AUTO-FIT to this field's range, and a solve yields propagation-TRANSFER levels
    // (negative dB — absolute L_den awaits the emission/SWL model, see the coverage-bound pin below).
    // So derive the edit from the live scale instead of hard-coding a dB value: pick a point strictly
    // between breaks[1] and breaks[2] so the ascending-order rule still holds and the edit is accepted.
    const before = await page.evaluate(() => window.__enviTest.colorScaleState());
    expect(before.breaks.length).toBeGreaterThan(3);
    const target = Number((((before.breaks[1] as number) + (before.breaks[2] as number)) / 2).toFixed(2));
    expect(target, "degenerate auto-fit breaks — cannot pick a strictly-between value").not.toBe(
      before.breaks[2],
    );

    // THE claim: editing the scale RE-CONTOURS the cached grid, it does not re-solve. Sample the job
    // lifecycle across the whole edit so a sneaky re-submit cannot hide behind the terminal `done`.
    const states = await recordJobStates(page, async () => {
      const breakInput = page.getByTestId("colorscale-break-2");
      await breakInput.fill(String(target));
      await breakInput.blur();
      await expect
        .poll(() => page.evaluate(() => window.__enviTest.colorScaleState().breaks[2]), { timeout: 15_000 })
        .toBe(target);
    });
    expectNoResolve(states, "editing the colour scale");

    // legend ≡ contour ≡ class: the SAME breaks[]/colors[] drive the tracer, the fill and the legend, so
    // there is exactly one colour class per interval (n breaks ⇒ n+1 classes).
    const scale = await page.evaluate(() => window.__enviTest.colorScaleState());
    const tel = await page.evaluate(() => window.__enviTest.isophoneTelemetry());
    expect(tel.traceCount, "the re-contour produced no polygons").toBeGreaterThan(0);
    expect(scale.colors.length).toBe(scale.breaks.length + 1);

    // Scene objects render at full styling ABOVE the isophone fill (D-18 draw order).
    //
    // The object-layer telemetry is written when those layers are (re-)applied, and these objects were
    // authored in step 4 — BEFORE the solve produced an isophone layer — so the snapshot still reads
    // `aboveIsophone: null` ("no isophone layer to compare against"). Nudge the store with a NO-OP patch to
    // force one re-apply now that the isophone exists: an empty patch yields a fresh feature reference
    // (layers re-apply) with byte-identical content, so the re-minted tensor identity is unchanged and the
    // results we still need in steps 9–11 cannot be falsely staled. (A basemap switch also re-applies, but
    // the object layers re-add BEFORE the isophone does, which just reproduces the stale `null`.)
    await page.evaluate((id) => window.__enviTest.update(id, {}), wallId);
    await expect
      .poll(async () => (await page.evaluate(() => window.__enviTest.objectLayerTelemetry())).aboveIsophone, {
        timeout: 30_000,
      })
      .toBe(true);
    // The no-op patch must not have staled the result (it would break conditioning + export below).
    expect(
      await page.evaluate(() => window.__enviTest.staleState().isStale),
      "a no-op property patch staled the tensor identity — identity is over-sensitive",
    ).toBe(false);

    const objects = await page.evaluate(() => window.__enviTest.objectLayerTelemetry());
    expect(objects.featureCount).toBeGreaterThan(0);
    // Belt-and-braces on the same claim, layer by layer (a single flag could mask one stray layer).
    const isoIdx = objects.layerOrder.indexOf("envi-isophone-fill");
    expect(isoIdx).toBeGreaterThanOrEqual(0);
    for (const layer of objects.registeredLayers) {
      expect(objects.layerOrder.indexOf(layer), `${layer} is not above the isophone fill`).toBeGreaterThan(
        isoIdx,
      );
    }

    // --- COVERAGE-BOUND PIN (see the header) -------------------------------------------------------
    // The solve marshals a flat homogeneous corridor: the objects authored in step 4 and the weather
    // derived in step 3 do NOT yet reach the solver. This journey therefore does NOT prove that a drawn
    // wall/forest/ground zone changes the solved level. When that marshalling lands, THIS ASSERTION
    // FAILS — that is intentional. Do not delete it: extend this journey to assert the objects actually
    // move the result, then update the pin. (ROADMAP Phase 11 / README "Known refinements".)
    const solvedFlat = await page.evaluate(() => {
      const m = window.__enviTest.isophoneTelemetry();
      return m.traceCount > 0;
    });
    expect(
      solvedFlat,
      "coverage-bound pin: the solve still runs on a flat homogeneous corridor (marshalScene emits " +
        "weather/forest/isolation = null). If terrain+weather marshalling has landed, extend this " +
        "journey to prove objects change the result.",
    ).toBe(true);
  });

  await test.step("9 — conditioning: a live MAC over the REAL solved tensor, with NO re-propagation", async () => {
    // The production results feed attached the real recondition/readout clients, so this drives the
    // genuine WASM MAC over the tensor the solve just wrote to OPFS — not a fixture.
    await expect(page.getByTestId("conditioning-panel")).toBeVisible();
    await expect(page.getByTestId("conditioning-stale-badge")).toHaveCount(0);

    const sourceId = await page.evaluate(() => window.__enviTest.conditioningState().order[0]);
    expect(sourceId, "the solve registered no conditioning source").toBeTruthy();

    const epoch0 = await page.evaluate(() => window.__enviTest.conditioningState().recalcEpoch);
    const tracesBefore = await page.evaluate(() => window.__enviTest.isophoneTelemetry().traceCount);
    expect(tracesBefore).toBeGreaterThan(0);

    // Gain, then delay (a phase ramp e^{−j2πfτ}) — each drives the debounced complex MAC over the cached
    // tensor, re-feeding BOTH the spectrum and the isophone map. Sample the job lifecycle across BOTH
    // edits: the flagship property is that this NEVER re-runs propagation.
    const states = await recordJobStates(page, async () => {
      await page.getByTestId(`conditioning-gain-${sourceId}`).fill("-9");
      await expect
        .poll(() => page.evaluate(() => window.__enviTest.conditioningState().recalcEpoch), {
          timeout: 30_000,
        })
        .toBeGreaterThan(epoch0);

      const epoch1 = await page.evaluate(() => window.__enviTest.conditioningState().recalcEpoch);
      await page.getByTestId(`conditioning-delay-${sourceId}`).fill("3");
      await expect
        .poll(() => page.evaluate(() => window.__enviTest.conditioningState().recalcEpoch), {
          timeout: 30_000,
        })
        .toBeGreaterThan(epoch1);

      await expect
        .poll(() => page.evaluate(() => window.__enviTest.isophoneTelemetry().traceCount), {
          timeout: 30_000,
        })
        .toBeGreaterThan(0);
    });
    expectNoResolve(states, "a conditioning edit");

    // Conditioning is excluded from the tensor identity (D-07), so it can never stale the result.
    expect(await page.evaluate(() => window.__enviTest.staleState().isStale)).toBe(false);
    await expect(page.getByTestId("conditioning-stale-badge")).toHaveCount(0);
  });

  await test.step("10 — scenarios: clone-then-edit, instant switch, and the A−B difference map", async () => {
    // The scenario registry runs on a seeded compute client (the documented fixture-driven surface —
    // see ROADMAP Phase 11 "Remaining refinement"); the DIFFERENCE math is real WASM.
    await page.evaluate(() => window.__enviTest.seedScenarios());
    await expect(page.getByTestId("scenario-panel")).toBeVisible();

    const epoch0 = await page.evaluate(() => window.__enviTest.scenarioState().computeEpoch);
    await page.getByTestId("scenario-compute-base").click();
    await expect(page.getByTestId("scenario-cached-base")).toBeVisible({ timeout: 30_000 });
    await expect
      .poll(() => page.evaluate(() => window.__enviTest.scenarioState().computeEpoch))
      .toBeGreaterThan(epoch0);

    // Clone-then-edit (D-13): a new scenario clones the active one and starts UNCOMPUTED — a met change
    // alters the tensor identity, so it must solve its own tensor rather than silently reusing base's.
    await page.getByTestId("scenario-new").click();
    await expect(page.getByTestId("scenario-empty")).toHaveCount(0);
    const afterClone = await page.evaluate(() => window.__enviTest.scenarioState());
    expect(afterClone.scenarios).toHaveLength(2);
    const cloneId = afterClone.scenarios.find((s) => s.id !== "base")?.id as string;
    expect(afterClone.activeId).toBe(cloneId);
    expect(
      afterClone.scenarios.find((s) => s.id === cloneId)?.computed,
      "a freshly cloned scenario claims to be computed — it would serve base's tensor",
    ).toBe(false);

    // Edit the friendly met (a warmer atmosphere), then compute THIS scenario's own cached tensor.
    await page.getByTestId("scenario-temp").fill("40");
    const epochBeforeClone = await page.evaluate(() => window.__enviTest.scenarioState().computeEpoch);
    await page.getByTestId(`scenario-compute-${cloneId}`).click();
    await expect(page.getByTestId(`scenario-cached-${cloneId}`)).toBeVisible({ timeout: 30_000 });
    await expect
      .poll(() => page.evaluate(() => window.__enviTest.scenarioState().computeEpoch), { timeout: 30_000 })
      .toBeGreaterThan(epochBeforeClone);

    // Instant switch between the two cached scenarios — a switch must NOT recompute.
    const beforeSwitch = await page.evaluate(() => window.__enviTest.scenarioState());
    await page.getByTestId("scenario-switch-base").click();
    await page.getByTestId(`scenario-switch-${cloneId}`).click();
    const afterSwitch = await page.evaluate(() => window.__enviTest.scenarioState());
    expect(afterSwitch.switchEpoch).toBeGreaterThan(beforeSwitch.switchEpoch);
    expect(
      afterSwitch.computeEpoch,
      "switching scenarios re-computed — per-scenario cached tensors are not being reused",
    ).toBe(beforeSwitch.computeEpoch);

    // The diverging difference map: A − B, gray at 0, fill polygons (never a raster).
    await page.getByTestId("scenario-compare-a").selectOption(cloneId);
    await page.getByTestId("scenario-compare-b").selectOption("base");
    await page.getByTestId("scenario-compare-run").click();
    await expect
      .poll(() => page.evaluate(() => window.__enviTest.differenceState().hasDelta), { timeout: 30_000 })
      .toBe(true);
    const diff = await page.evaluate(() => window.__enviTest.differenceTelemetry());
    expect(diff.featureCount).toBeGreaterThan(0);
    await expect(page.getByTestId("difference-legend")).toBeVisible();
  });

  await test.step("11 — export: GeoTIFF + GeoJSON + CSV download locally, with attribution", async () => {
    // GeoTIFF — a real little-endian TIFF carrying the CRS + open-data attribution footer.
    {
      const { download, bytes } = await grabExport(page, "geotiff");
      expect(download.suggestedFilename()).toMatch(/\.tif$/);
      expect(bytes[0]).toBe(0x49); // 'I'
      expect(bytes[1]).toBe(0x49); // 'I' → little-endian TIFF magic
      expect(bytes.toString("latin1")).toContain("OpenStreetMap");
    }
    // GeoJSON — RFC-7946 isophone polygons.
    {
      const { download, bytes } = await grabExport(page, "geojson");
      expect(download.suggestedFilename()).toMatch(/\.geojson$/);
      const text = bytes.toString("utf-8");
      const fc = JSON.parse(text) as { type: string; features: unknown[] };
      expect(fc.type).toBe("FeatureCollection");
      expect(fc.features.length).toBeGreaterThan(0);
      expect(text).toContain("OpenStreetMap");
    }
    // CSV — the band identity is BAND INDEX + exact Hz (never nominal Hz alone).
    {
      const { download, bytes } = await grabExport(page, "csv");
      expect(download.suggestedFilename()).toMatch(/\.csv$/);
      const text = bytes.toString("utf-8");
      expect(text).toContain("band_index,exact_hz");
      expect(text).toContain("dBA_total");
      expect(text).toContain("# Attribution:");
    }
  });

  await test.step("12 — the honest stale/409 states: a scene edit refuses to serve a stale readout", async () => {
    // Mutate the scene so its re-minted tensor identity diverges from the cached tensor.
    await page.evaluate(() => window.__enviTest.divergeScene());

    // The results-stale badge flips the moment the scene diverges (D-12)…
    await expect(page.getByTestId("conditioning-stale-badge")).toBeVisible({ timeout: 15_000 });
    expect(await page.evaluate(() => window.__enviTest.staleState().isStale)).toBe(true);

    // …a MAC against the mismatched hash is REFUSED, never silently served (the SVC-06 409)…
    const sourceId = await page.evaluate(() => window.__enviTest.conditioningState().order[0]);
    await page.getByTestId(`conditioning-gain-${sourceId}`).fill("-3");
    await expect(page.getByTestId("conditioning-reject")).toBeVisible({ timeout: 30_000 });
    await expect
      .poll(() => page.evaluate(() => window.__enviTest.conditioningState().refuse), { timeout: 15_000 })
      .toBe(true);

    // …and a stale result cannot be exported.
    await expect(page.getByTestId("export-open")).toBeDisabled();
  });

  await test.step("13 — INVARIANT: the entire session touched the network ZERO times", async () => {
    expect(
      unmocked,
      `the session made live network requests: ${unmocked.join(", ")}`,
    ).toEqual([]);
  });
});
