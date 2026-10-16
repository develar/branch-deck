import { z } from "zod"
import { useStore } from "zustand"
import { createPersistentStore } from "./persistent-store"
import type { ArrayStoreState } from "./persistent-store"
import { stableStringify } from "./sync-utils"

// Schema for ProjectMetadata with normalization
const ProjectMetadataSchema = z.object({
  path: z.string(),
  cachedBranchPrefix: z.string().optional().transform(val => val === "" ? undefined : val),
  lastSyncTime: z.number().optional(),
  /** When Update Project last fetched the baseline (ms); a fetch elsewhere shows through FETCH_HEAD */
  lastFetchTime: z.number().optional(),
  lastBranchCount: z.number().optional(),
  /**
   * The Landed island hides the branches that landed up to this time (`landedAt`, Unix seconds): a sync hid them after
   * the user had seen them. Their chips stay until Dismiss.
   */
  landedHiddenUntil: z.number().optional(),
  /** The user collapsed the "Your Turn" card of the repository (open by default) */
  reviewRequestsCollapsed: z.boolean().optional(),
  issueNavigationConfig: z.any().optional(), // TODO: create proper schema
})

// Schema for repository settings - just the array directly
const RepositorySettingsSchema = z.array(ProjectMetadataSchema).default([])

export type ProjectMetadata = z.infer<typeof ProjectMetadataSchema>

/** Recent projects, most recently selected first. `items[0]` is the selected project. */
export const repositorySettingsStore = createPersistentStore(
  "recentProjects", // Keep the same key for backward compatibility
  { schema: RepositorySettingsSchema, isArray: true, isMainOnly: false },
)

export function useRecentProjects<T>(selector: (projects: ProjectMetadata[]) => T): T {
  return useStore(repositorySettingsStore, (state: ArrayStoreState<ProjectMetadata>) => selector(state.items))
}

export function getRecentProjects(): ProjectMetadata[] {
  return repositorySettingsStore.getState().items
}

export function setRecentProjects(items: ProjectMetadata[]): void {
  repositorySettingsStore.setState({ items })
}

function isSameValue(a: unknown, b: unknown): boolean {
  return Object.is(a, b) || (typeof a === "object" && typeof b === "object" && a !== null && b !== null && stableStringify(a) === stableStringify(b))
}

/** Immutably updates the project with the given path; no-op if it is not in the list. */
export function updateProject(path: string, update: Partial<ProjectMetadata> | ((project: ProjectMetadata) => ProjectMetadata)): void {
  const items = getRecentProjects()
  const index = items.findIndex(it => it.path === path)
  if (index === -1) {
    return
  }

  const current = items[index]!
  // the same values keep the record: every row selects from it, and the sync sends the issue links every time
  if (typeof update !== "function" && Object.entries(update).every(([key, value]) => isSameValue(current[key as keyof ProjectMetadata], value))) {
    return
  }
  const next = typeof update === "function" ? update(current) : { ...current, ...update }
  if (next === current) {
    return
  }
  setRecentProjects(items.with(index, next))
}

/** Collapses or opens the "Your Turn" card of the repository; open is the default and is not stored */
export function setReviewRequestsCollapsed(path: string, collapsed: boolean): void {
  updateProject(path, project => (project.reviewRequestsCollapsed === true) === collapsed ? project : { ...project, reviewRequestsCollapsed: collapsed ? true : undefined })
}
