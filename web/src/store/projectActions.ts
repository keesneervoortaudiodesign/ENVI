// projectActions.ts — the project lifecycle orchestration seam (WEB-01, D-06): open / create / reopen-last.
// These are the REAL actions the 07-09 placeholder Open/New buttons and the app boot now call; they compose
// the typed client (`api/client`) with the canonical store (`setProject` + `loadScene`).
//
// # Module I/O
// - Input  a project id (open), a name + WGS84 origin (create), or nothing (reopen-last). Each fetches the
//   project's metadata + persisted scene and hydrates the canonical store so the map re-renders (SC4).
// - Output the store transitions: `openProjectById`/`reopenLast` set the project identity and bulk-load the
//   scene (bumping `loadEpoch` so Terra Draw re-hydrates); `createAndOpen` creates then opens the new
//   (empty) project. `reopenLast` resolves to `false` (never throws) when there is no last project — the
//   boot path treats that as the ordinary "no project open" empty state.
// - Valid input range: `id` is a project uuid; `origin` is WGS84 degrees. The scene wire is geometry-only
//   (isolation spectra are not carried by PUT /scene, so a reopened project starts with no spectra).

import {
  createProject,
  getLastProject,
  getProject,
  getScene,
  listProjects,
} from "../api/client";
import { useSceneStore } from "./sceneStore";
import type { OriginDto, ProjectMetaDto } from "../generated/wire";

// Set the project identity and bulk-load its persisted scene into the canonical store (one open path shared
// by open / reopen-last). Terra Draw re-hydrates off the resulting `loadEpoch` bump.
async function hydrateProject(meta: ProjectMetaDto): Promise<void> {
  const scene = await getScene(meta.id);
  const store = useSceneStore.getState();
  store.setProject(meta.id, meta.name);
  store.loadScene(scene);
}

// Open a project by id (the Open picker): fetch its metadata (records reopen-last server-side) + scene.
export async function openProjectById(id: string): Promise<void> {
  const meta = await getProject(id);
  await hydrateProject(meta);
}

// Reopen the last-opened project on app boot (D-06). Returns whether a project was restored; a missing
// last-project (404 / id-less stub) is the normal empty state, so this NEVER throws to the boot caller.
export async function reopenLast(): Promise<boolean> {
  try {
    const meta = await getLastProject();
    if (!meta) {
      return false;
    }
    await hydrateProject(meta);
    return true;
  } catch {
    return false;
  }
}

// Create a project at `origin` and open it (the "New project" flow). The new project's scene is empty.
export async function createAndOpen(name: string, origin: OriginDto): Promise<void> {
  const meta = await createProject({ name, description: null, origin });
  await hydrateProject(meta);
}

// The project list for the Open picker (metadata only; the scene is fetched on open).
export function listProjectMetas(): Promise<ProjectMetaDto[]> {
  return listProjects();
}
