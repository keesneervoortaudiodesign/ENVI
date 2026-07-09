//! Contract tests: project CRUD + reopen-last, restart-survival (SC1), and the
//! validate-before-persist scene guard — oneshot against the FULL app router,
//! TempDir roots, no sockets (06-RESEARCH Validation Architecture).

use std::collections::BTreeMap;

use axum::http::{Method, StatusCode};
use tower::ServiceExt; // oneshot

mod common;
use common::{app_over, get, json_req, read_json};

/// A FeatureCollection with one feature of each of the 9 kinds (WGS84, Amsterdam).
fn nine_kind_scene() -> serde_json::Value {
    serde_json::json!({
      "type": "FeatureCollection",
      "features": [
        {"type":"Feature","geometry":{"type":"Point","coordinates":[4.8936,52.3731]},
         "properties":{"kind":"source","id":"00000000-0000-0000-0000-000000000001","height_m":0.5}},
        {"type":"Feature","geometry":{"type":"Point","coordinates":[4.8950,52.3740]},
         "properties":{"kind":"receiver","id":"00000000-0000-0000-0000-000000000002","height_m":1.5}},
        {"type":"Feature","geometry":{"type":"LineString","coordinates":[[4.8940,52.3733],[4.8945,52.3736]]},
         "properties":{"kind":"wall","id":"00000000-0000-0000-0000-000000000003","height_m":3.0}},
        {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[4.8930,52.3730],[4.8933,52.3730],[4.8933,52.3733],[4.8930,52.3730]]]},
         "properties":{"kind":"building","id":"00000000-0000-0000-0000-000000000004","eaves_height_m":6.0}},
        {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[4.8960,52.3750],[4.8963,52.3750],[4.8963,52.3753],[4.8960,52.3750]]]},
         "properties":{"kind":"forest","id":"00000000-0000-0000-0000-000000000005"}},
        {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[4.8920,52.3720],[4.8923,52.3720],[4.8923,52.3723],[4.8920,52.3720]]]},
         "properties":{"kind":"ground_zone","id":"00000000-0000-0000-0000-000000000006","impedance_class":"D"}},
        {"type":"Feature","geometry":{"type":"Point","coordinates":[4.8910,52.3710]},
         "properties":{"kind":"elevation_point","id":"00000000-0000-0000-0000-000000000007","z_m":2.0}},
        {"type":"Feature","geometry":{"type":"LineString","coordinates":[[4.8900,52.3700],[4.8905,52.3702]]},
         "properties":{"kind":"elevation_line","id":"00000000-0000-0000-0000-000000000008"}},
        {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[4.8970,52.3760],[4.8973,52.3760],[4.8973,52.3763],[4.8970,52.3760]]]},
         "properties":{"kind":"calc_area","id":"00000000-0000-0000-0000-000000000009"}}
      ]
    })
}

/// Index a FeatureCollection's features by `properties.id` for equality checks.
fn features_by_id(fc: &serde_json::Value) -> BTreeMap<String, serde_json::Value> {
    fc["features"]
        .as_array()
        .expect("features array")
        .iter()
        .map(|f| {
            let id = f["properties"]["id"]
                .as_str()
                .expect("feature id")
                .to_string();
            (id, f.clone())
        })
        .collect()
}

#[tokio::test]
async fn crud_lifecycle_and_reopen_last() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let app = app_over(tmp.path());

    // create (Dam Square origin) -> 201 + pinned CRS zone 31, south false.
    let create_body = serde_json::json!({
        "name": "My Scene",
        "description": "desc",
        "origin": { "lon_deg": 4.8936, "lat_deg": 52.3731 }
    });
    let (status, created) = read_json(
        app.clone()
            .oneshot(json_req(Method::POST, "/api/v1/projects", &create_body))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "POST /projects is 201");
    assert_eq!(
        created["crs"]["utm_zone"].as_u64(),
        Some(31),
        "pinned zone 31"
    );
    assert_eq!(created["crs"]["south"].as_bool(), Some(false), "north");
    let id = created["id"].as_str().expect("project id").to_string();

    // list shows 1.
    let (status, listed) =
        read_json(app.clone().oneshot(get("/api/v1/projects")).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed.as_array().unwrap().len(), 1, "one project listed");

    // GET {id} -> 200 (also records reopen-last).
    let (status, _) = read_json(
        app.clone()
            .oneshot(get(&format!("/api/v1/projects/{id}")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GET project is 200");

    // GET /projects/last -> same id.
    let (status, last) = read_json(
        app.clone()
            .oneshot(get("/api/v1/projects/last"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        last["id"].as_str(),
        Some(id.as_str()),
        "reopen-last matches"
    );

    // PUT metadata rename -> 200, name changed on re-GET.
    let rename = serde_json::json!({ "name": "Renamed Scene" });
    let (status, updated) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::PUT,
                &format!("/api/v1/projects/{id}"),
                &rename,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "PUT metadata is 200");
    assert_eq!(updated["name"].as_str(), Some("Renamed Scene"), "renamed");
    let (_, re_got) = read_json(
        app.clone()
            .oneshot(get(&format!("/api/v1/projects/{id}")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        re_got["name"].as_str(),
        Some("Renamed Scene"),
        "rename persisted"
    );

    // duplicate -> 201 new id; list shows 2; the duplicate has no calc/.
    let (status, dup) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::POST,
                &format!("/api/v1/projects/{id}/duplicate"),
                &serde_json::json!(null),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "duplicate is 201");
    let dup_id = dup["id"].as_str().expect("dup id").to_string();
    assert_ne!(dup_id, id, "duplicate gets a new id");
    assert!(
        !tmp.path().join(&dup_id).join("calc").exists(),
        "duplicated folder has no calc/"
    );
    let (_, listed2) = read_json(app.clone().oneshot(get("/api/v1/projects")).await.unwrap()).await;
    assert_eq!(
        listed2.as_array().unwrap().len(),
        2,
        "two projects after dup"
    );

    // DELETE original -> 204; list shows 1; GET deleted -> 404 JSON error body.
    let resp = app
        .clone()
        .oneshot(json_req(
            Method::DELETE,
            &format!("/api/v1/projects/{id}"),
            &serde_json::json!(null),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "DELETE is 204");

    let (_, listed3) = read_json(app.clone().oneshot(get("/api/v1/projects")).await.unwrap()).await;
    assert_eq!(
        listed3.as_array().unwrap().len(),
        1,
        "one project after delete"
    );

    let (status, err_body) = read_json(
        app.clone()
            .oneshot(get(&format!("/api/v1/projects/{id}")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "GET deleted -> 404");
    assert_eq!(
        err_body["error"], "not_found",
        "404 body is structured JSON"
    );
}

#[tokio::test]
async fn project_round_trips_across_restart() {
    // SC1 restart-survival proof: create + PUT scene under AppState A, drop it,
    // rebuild AppState B over the SAME directory, GET scene, assert identity.
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let scene = nine_kind_scene();

    let id = {
        let app_a = app_over(tmp.path());

        let create_body = serde_json::json!({
            "name": "Restart Scene",
            "origin": { "lon_deg": 4.8936, "lat_deg": 52.3731 }
        });
        let (status, created) = read_json(
            app_a
                .clone()
                .oneshot(json_req(Method::POST, "/api/v1/projects", &create_body))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().expect("id").to_string();

        // PUT the 9-kind scene -> 204.
        let resp = app_a
            .clone()
            .oneshot(json_req(
                Method::PUT,
                &format!("/api/v1/projects/{id}/scene"),
                &scene,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT, "PUT scene is 204");
        id
        // app_a dropped here — simulates process shutdown.
    };

    // Fresh AppState over the same root (a "restarted" service).
    let app_b = app_over(tmp.path());

    // reopen-last still resolves after restart.
    let (status, last) = read_json(
        app_b
            .clone()
            .oneshot(get("/api/v1/projects/last"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "reopen-last resolves after restart");
    assert_eq!(last["id"].as_str(), Some(id.as_str()));

    // GET scene -> the identical FeatureCollection.
    let (status, got) = read_json(
        app_b
            .clone()
            .oneshot(get(&format!("/api/v1/projects/{id}/scene")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "GET scene is 200 after restart");

    let put_features = features_by_id(&scene);
    let got_features = features_by_id(&got);
    assert_eq!(
        got_features.len(),
        9,
        "all 9 features survive the restart round-trip"
    );
    assert_eq!(
        got_features.keys().collect::<Vec<_>>(),
        put_features.keys().collect::<Vec<_>>(),
        "same feature id set"
    );
    for (id, put_f) in &put_features {
        let got_f = &got_features[id];
        assert_eq!(
            got_f["properties"]["kind"], put_f["properties"]["kind"],
            "kind preserved for {id}"
        );
        assert_eq!(
            got_f["geometry"]["coordinates"], put_f["geometry"]["coordinates"],
            "coordinates preserved for {id}"
        );
    }
}

#[tokio::test]
async fn scene_put_rejects_invalid() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let app = app_over(tmp.path());

    let create_body = serde_json::json!({
        "name": "Guard Scene",
        "origin": { "lon_deg": 4.8936, "lat_deg": 52.3731 }
    });
    let (_, created) = read_json(
        app.clone()
            .oneshot(json_req(Method::POST, "/api/v1/projects", &create_body))
            .await
            .unwrap(),
    )
    .await;
    let id = created["id"].as_str().expect("id").to_string();

    // First PUT a valid scene so there is a known-good persisted state.
    let good = nine_kind_scene();
    let resp = app
        .clone()
        .oneshot(json_req(
            Method::PUT,
            &format!("/api/v1/projects/{id}/scene"),
            &good,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "valid scene saved");

    // PUT an out-of-vocabulary kind -> 400 naming the kind.
    let bad = serde_json::json!({
        "type": "FeatureCollection",
        "features": [
            {"type":"Feature","geometry":{"type":"Point","coordinates":[4.89,52.37]},
             "properties":{"kind":"teleporter","id":"00000000-0000-0000-0000-0000000000aa"}}
        ]
    });
    let (status, err_body) = read_json(
        app.clone()
            .oneshot(json_req(
                Method::PUT,
                &format!("/api/v1/projects/{id}/scene"),
                &bad,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "invalid kind -> 400");
    let detail = err_body["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("teleporter"),
        "error names the offending kind: {detail}"
    );

    // GET returns the PREVIOUS (valid, 9-kind) scene — bad input never persisted.
    let (status, got) = read_json(
        app.clone()
            .oneshot(get(&format!("/api/v1/projects/{id}/scene")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        got["features"].as_array().unwrap().len(),
        9,
        "the previous valid scene is unchanged (bad PUT never reached disk)"
    );
}
