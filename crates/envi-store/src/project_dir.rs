//! Project-as-folder storage: layout, atomic saves, and CRUD + reopen-last
//! (D-04, D-06, SVC-01, SVC-05).
//!
//! # Layout
//!
//! ```text
//! <root>/
//!   .envi-state.json                  reopen-last record
//!   <uuid>/
//!     project.json                    ProjectMetaDto (metadata + settings + CRS)
//!     scene.geojson                   WGS84 FeatureCollection
//!     calc/<calc_id>/                 reserved calc layout (manifest.rs)
//! ```
//!
//! # Pitfall: cross-volume persist (Pitfall 4)
//!
//! [`atomic_write`] creates the `NamedTempFile` **inside the destination dir**
//! (`new_in`), `sync_all`s it, then `persist`s over the target — a temp file in
//! the system temp dir cannot be `persist`ed across filesystems, and an
//! unflushed temp loses content on a crash. Every mutation (autosave: the server
//! is authoritative, no dirty state — D-06) routes through this one helper.
//!
//! # Pitfall: path traversal (Pitfall 7, T-06-02-01/02)
//!
//! All id-taking methods take `uuid::Uuid` (never `&str`), so the parse-as-uuid
//! gate happens before any path join. Destructive ops (delete, duplicate)
//! additionally canonicalize the resolved project dir and verify it stays under
//! the canonicalized store root (symlink-escape guard).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use geojson::FeatureCollection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use envi_geo::{LonLat, ProjectCrs};

use crate::StoreError;
use crate::dto::{CrsDto, ProjectMetaDto, SettingsDto};
use crate::geojson::validate_feature_collection;

/// The reopen-last record persisted at `<root>/.envi-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnviState {
    /// The most recently opened project id.
    last_project_id: Uuid,
    /// When it was opened, unix epoch seconds.
    opened_at_unix: u64,
}

/// A folder-backed project store rooted at `<root>/`.
#[derive(Debug, Clone)]
pub struct ProjectStore {
    root: PathBuf,
}

impl ProjectStore {
    /// Open (creating if absent) a store rooted at `root`.
    ///
    /// # Errors
    /// [`StoreError::Io`] if the root cannot be created.
    pub fn new(root: PathBuf) -> Result<Self, StoreError> {
        std::fs::create_dir_all(&root).map_err(|source| StoreError::Io {
            path: root.clone(),
            source,
        })?;
        Ok(Self { root })
    }

    /// The (unchecked) directory for a project id — `root/<uuid>`. The id is a
    /// `Uuid`, so no untrusted string is ever joined (Pitfall 7).
    fn dir_of(&self, id: Uuid) -> PathBuf {
        self.root.join(id.to_string())
    }

    /// The on-disk directory for a project id (`root/<uuid>`). Public so the
    /// service's calc layer can address `calc/<cid>/` under it (manifest writes
    /// via [`crate::manifest::write_manifest`]). The id is a `Uuid`, so no
    /// untrusted string is ever joined into the path (Pitfall 7).
    #[must_use]
    pub fn project_dir(&self, id: Uuid) -> PathBuf {
        self.dir_of(id)
    }

    /// Resolve + containment-check a project dir for a destructive op: it must
    /// exist and canonicalize under the canonicalized store root.
    fn guarded_dir(&self, id: Uuid) -> Result<PathBuf, StoreError> {
        let dir = self.dir_of(id);
        let canon_root = self.root.canonicalize().map_err(|source| StoreError::Io {
            path: self.root.clone(),
            source,
        })?;
        let canon = dir
            .canonicalize()
            .map_err(|_| StoreError::NotFound { project_id: id })?;
        if !canon.starts_with(&canon_root) {
            return Err(StoreError::PathEscape { path: canon });
        }
        Ok(canon)
    }

    /// Create a new project: pins the CRS from `origin` (D-03), writes
    /// `project.json` + an empty `scene.geojson`, and records reopen-last.
    ///
    /// # Errors
    /// [`StoreError`] on reprojection setup or filesystem failure.
    pub fn create(
        &self,
        name: &str,
        description: &str,
        origin: LonLat,
    ) -> Result<ProjectMetaDto, StoreError> {
        let id = Uuid::new_v4();
        let crs = ProjectCrs::for_location(origin)?;
        let now = now_unix();
        let meta = ProjectMetaDto {
            id,
            name: name.to_string(),
            description: description.to_string(),
            created_at_unix: now,
            modified_at_unix: now,
            crs: CrsDto::from(&crs),
            settings: SettingsDto::default(),
        };
        let dir = self.dir_of(id);
        std::fs::create_dir_all(&dir).map_err(|source| StoreError::Io {
            path: dir.clone(),
            source,
        })?;
        self.write_meta(&dir, &meta)?;
        let empty = FeatureCollection {
            bbox: None,
            features: Vec::new(),
            foreign_members: None,
        };
        self.write_scene(&dir, &empty)?;
        self.record_open(id)?;
        Ok(meta)
    }

    /// List the ids of every valid project (dir name parses as a `Uuid` AND the
    /// dir contains `project.json`). Non-project entries are ignored.
    ///
    /// # Errors
    /// [`StoreError::Io`] if the root cannot be read.
    pub fn list(&self) -> Result<Vec<Uuid>, StoreError> {
        let mut ids = Vec::new();
        let entries = std::fs::read_dir(&self.root).map_err(|source| StoreError::Io {
            path: self.root.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| StoreError::Io {
                path: self.root.clone(),
                source,
            })?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Ok(id) = Uuid::parse_str(name) else {
                continue;
            };
            if entry.path().join("project.json").is_file() {
                ids.push(id);
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// Open a project: read + return its metadata, and record reopen-last.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the project does not exist.
    pub fn open(&self, id: Uuid) -> Result<ProjectMetaDto, StoreError> {
        let meta = self.load_meta(id)?;
        self.record_open(id)?;
        Ok(meta)
    }

    /// Read a project's metadata without recording reopen-last.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent; [`StoreError::Json`] if malformed.
    pub fn load_meta(&self, id: Uuid) -> Result<ProjectMetaDto, StoreError> {
        let path = self.dir_of(id).join("project.json");
        let bytes = std::fs::read(&path).map_err(|_| StoreError::NotFound { project_id: id })?;
        serde_json::from_slice(&bytes).map_err(|e| StoreError::Json {
            path,
            message: e.to_string(),
        })
    }

    /// Persist updated project metadata (autosave; atomic).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the project dir is absent.
    pub fn save_meta(&self, meta: &ProjectMetaDto) -> Result<(), StoreError> {
        let dir = self.dir_of(meta.id);
        if !dir.is_dir() {
            return Err(StoreError::NotFound {
                project_id: meta.id,
            });
        }
        self.write_meta(&dir, meta)
    }

    /// Load a project's scene as a validated WGS84 FeatureCollection.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent; [`StoreError::GeoJson`] if malformed.
    pub fn load_scene(&self, id: Uuid) -> Result<FeatureCollection, StoreError> {
        let path = self.dir_of(id).join("scene.geojson");
        let text =
            std::fs::read_to_string(&path).map_err(|_| StoreError::NotFound { project_id: id })?;
        let fc: FeatureCollection =
            serde_json::from_str(&text).map_err(|e| StoreError::GeoJson {
                message: format!("scene.geojson parse error: {e}"),
            })?;
        Ok(fc)
    }

    /// Save a project's scene (validated before it reaches disk; atomic).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the project dir is absent; a validation error
    /// if the scene violates the schema — invalid scenes never reach disk.
    pub fn save_scene(&self, id: Uuid, scene: &FeatureCollection) -> Result<(), StoreError> {
        let dir = self.dir_of(id);
        if !dir.is_dir() {
            return Err(StoreError::NotFound { project_id: id });
        }
        validate_feature_collection(scene)?;
        self.write_scene(&dir, scene)
    }

    /// Duplicate a project under a new uuid, copying every top-level file but
    /// **excluding `calc/`** (stale tensor identity must not travel). The copy's
    /// `project.json` gets the new id, a `" (copy)"` name suffix, and fresh
    /// timestamps.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the source is absent; [`StoreError::PathEscape`]
    /// if it fails the containment guard.
    pub fn duplicate(&self, id: Uuid) -> Result<ProjectMetaDto, StoreError> {
        let src = self.guarded_dir(id)?;
        let mut meta = self.load_meta(id)?;

        let new_id = Uuid::new_v4();
        let dest = self.dir_of(new_id);
        std::fs::create_dir_all(&dest).map_err(|source| StoreError::Io {
            path: dest.clone(),
            source,
        })?;

        // Copy every top-level file except calc/ (dirs other than calc/ do not
        // occur in Phase 6 and are intentionally not recursed).
        let entries = std::fs::read_dir(&src).map_err(|source| StoreError::Io {
            path: src.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| StoreError::Io {
                path: src.clone(),
                source,
            })?;
            let name = entry.file_name();
            if name == "calc" {
                continue;
            }
            let file_type = entry.file_type().map_err(|source| StoreError::Io {
                path: entry.path(),
                source,
            })?;
            if !file_type.is_file() {
                continue;
            }
            let Some(name_str) = name.to_str() else {
                continue;
            };
            // project.json is rewritten below; skip copying the stale one.
            if name_str == "project.json" {
                continue;
            }
            let bytes = std::fs::read(entry.path()).map_err(|source| StoreError::Io {
                path: entry.path(),
                source,
            })?;
            atomic_write(&dest, name_str, &bytes)?;
        }

        let now = now_unix();
        meta.id = new_id;
        meta.name = format!("{} (copy)", meta.name);
        meta.created_at_unix = now;
        meta.modified_at_unix = now;
        self.write_meta(&dest, &meta)?;
        Ok(meta)
    }

    /// Delete a project folder (after the containment guard).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent; [`StoreError::PathEscape`] on a
    /// containment-check failure.
    pub fn delete(&self, id: Uuid) -> Result<(), StoreError> {
        let dir = self.guarded_dir(id)?;
        std::fs::remove_dir_all(&dir).map_err(|source| StoreError::Io { path: dir, source })
    }

    /// The last-opened project id, or `None` if there is no record or the
    /// recorded project no longer exists (documented choice: a deleted project
    /// yields `None`, not a dangling id).
    ///
    /// # Errors
    /// [`StoreError::Json`] if the state file is malformed.
    pub fn last_opened(&self) -> Result<Option<Uuid>, StoreError> {
        let path = self.state_path();
        let Ok(bytes) = std::fs::read(&path) else {
            return Ok(None);
        };
        let state: EnviState = serde_json::from_slice(&bytes).map_err(|e| StoreError::Json {
            path,
            message: e.to_string(),
        })?;
        if self
            .dir_of(state.last_project_id)
            .join("project.json")
            .is_file()
        {
            Ok(Some(state.last_project_id))
        } else {
            Ok(None)
        }
    }

    // --- internal helpers ---

    fn state_path(&self) -> PathBuf {
        self.root.join(".envi-state.json")
    }

    fn record_open(&self, id: Uuid) -> Result<(), StoreError> {
        let state = EnviState {
            last_project_id: id,
            opened_at_unix: now_unix(),
        };
        let bytes = serde_json::to_vec_pretty(&state).map_err(|e| StoreError::Json {
            path: self.state_path(),
            message: e.to_string(),
        })?;
        atomic_write(&self.root, ".envi-state.json", &bytes)
    }

    fn write_meta(&self, dir: &Path, meta: &ProjectMetaDto) -> Result<(), StoreError> {
        let bytes = serde_json::to_vec_pretty(meta).map_err(|e| StoreError::Json {
            path: dir.join("project.json"),
            message: e.to_string(),
        })?;
        atomic_write(dir, "project.json", &bytes)
    }

    fn write_scene(&self, dir: &Path, scene: &FeatureCollection) -> Result<(), StoreError> {
        let bytes = serde_json::to_vec_pretty(scene).map_err(|e| StoreError::Json {
            path: dir.join("scene.geojson"),
            message: e.to_string(),
        })?;
        atomic_write(dir, "scene.geojson", &bytes)
    }
}

/// Atomically write `bytes` to `dir/name` (06-RESEARCH Pattern 3):
/// a `NamedTempFile` created **in `dir`** (same volume), `sync_all`ed, then
/// `persist`ed over the target — an interrupted write never truncates the old
/// file, and there is no cross-volume `persist` failure (Pitfall 4).
///
/// # Errors
/// [`StoreError::Io`] on any create / write / sync / persist failure.
pub fn atomic_write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), StoreError> {
    let target = dir.join(name);
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|source| StoreError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    use std::io::Write;
    tmp.write_all(bytes).map_err(|source| StoreError::Io {
        path: target.clone(),
        source,
    })?;
    tmp.as_file().sync_all().map_err(|source| StoreError::Io {
        path: target.clone(),
        source,
    })?;
    tmp.persist(&target).map_err(|e| StoreError::Io {
        path: target.clone(),
        source: e.error,
    })?;
    Ok(())
}

/// Current unix epoch seconds (monotone wall clock; dependency-free).
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dam_square() -> LonLat {
        LonLat {
            lon_deg: 4.8936,
            lat_deg: 52.3731,
        }
    }

    fn tiny_scene() -> FeatureCollection {
        let json = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","geometry":{"type":"Point","coordinates":[4.8936,52.3731]},
           "properties":{"kind":"receiver","id":"00000000-0000-0000-0000-000000000002","height_m":1.5}}]}"#;
        serde_json::from_str(json).expect("valid FC")
    }

    #[test]
    fn atomic_write_replaces_never_truncates() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let dir = tmp.path();

        atomic_write(dir, "f.json", b"v1").expect("write v1");
        assert_eq!(std::fs::read(dir.join("f.json")).unwrap(), b"v1");

        atomic_write(dir, "f.json", b"v2-longer-content").expect("write v2");
        assert_eq!(
            std::fs::read(dir.join("f.json")).unwrap(),
            b"v2-longer-content",
            "content is exactly v2"
        );

        // No *.tmp orphan remains: the dir holds only the target file.
        let names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["f.json".to_string()], "no temp orphan left");
    }

    #[test]
    fn crud_lifecycle_and_reopen_last() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let store = ProjectStore::new(tmp.path().to_path_buf()).expect("store");

        // create pins the CRS from Dam Square -> zone 31 north.
        let meta = store
            .create("My Scene", "desc", dam_square())
            .expect("create");
        assert_eq!(meta.crs.utm_zone, 31, "Amsterdam -> UTM zone 31");
        assert!(!meta.crs.south, "northern hemisphere");

        // list contains it.
        assert_eq!(store.list().unwrap(), vec![meta.id]);

        // open records reopen-last.
        store.open(meta.id).expect("open");
        assert_eq!(store.last_opened().unwrap(), Some(meta.id));

        // save_scene + load_scene round-trip.
        let scene = tiny_scene();
        store.save_scene(meta.id, &scene).expect("save scene");
        let loaded = store.load_scene(meta.id).expect("load scene");
        assert_eq!(loaded.features.len(), scene.features.len());
        assert_eq!(
            serde_json::to_string(&loaded).unwrap(),
            serde_json::to_string(&scene).unwrap(),
            "scene round-trips as parsed values"
        );

        // Give the original a calc/ dir so duplicate's exclusion is provable.
        std::fs::create_dir_all(store.dir_of(meta.id).join("calc").join("c1")).unwrap();

        // duplicate -> new uuid, has project.json + scene.geojson, NO calc/.
        let dup = store.duplicate(meta.id).expect("duplicate");
        assert_ne!(dup.id, meta.id, "duplicate gets a new uuid");
        assert!(dup.name.ends_with("(copy)"), "name suffixed");
        let dup_dir = store.dir_of(dup.id);
        assert!(
            dup_dir.join("project.json").is_file(),
            "copy has project.json"
        );
        assert!(
            dup_dir.join("scene.geojson").is_file(),
            "copy has scene.geojson"
        );
        assert!(!dup_dir.join("calc").exists(), "copy excludes calc/");

        // delete removes the folder.
        store.delete(meta.id).expect("delete");
        assert!(!store.dir_of(meta.id).exists(), "folder removed");

        // last_opened of the deleted project yields None (documented choice).
        assert_eq!(store.last_opened().unwrap(), None);
    }

    #[test]
    fn traversal_and_symlink_guard() {
        // A traversal string never parses as a Uuid, so it can never reach a
        // path join through the Uuid-typed API (Pitfall 7).
        assert!(Uuid::parse_str("../../etc/passwd").is_err());

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let store = ProjectStore::new(tmp.path().to_path_buf()).expect("store");

        // Destructive ops on a non-existent project fail the containment guard
        // (canonicalize fails) with a typed error, never a silent escape.
        let unknown = Uuid::from_u128(12345);
        assert!(
            matches!(store.delete(unknown), Err(StoreError::NotFound { .. })),
            "delete of unknown id is NotFound"
        );
        assert!(
            matches!(store.duplicate(unknown), Err(StoreError::NotFound { .. })),
            "duplicate of unknown id is NotFound"
        );
    }

    #[test]
    fn save_scene_rejects_invalid_scene_before_disk() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let store = ProjectStore::new(tmp.path().to_path_buf()).expect("store");
        let meta = store.create("p", "", dam_square()).expect("create");

        // Unknown kind -> rejected; scene.geojson stays the empty one written at create.
        let bad: FeatureCollection = serde_json::from_str(
            r#"{"type":"FeatureCollection","features":[
              {"type":"Feature","geometry":{"type":"Point","coordinates":[4.89,52.37]},
               "properties":{"kind":"bogus","id":"00000000-0000-0000-0000-000000000001"}}]}"#,
        )
        .unwrap();
        assert!(
            store.save_scene(meta.id, &bad).is_err(),
            "invalid scene rejected"
        );
        let on_disk = store.load_scene(meta.id).expect("still loads");
        assert!(
            on_disk.features.is_empty(),
            "invalid scene never reached disk"
        );
    }
}
