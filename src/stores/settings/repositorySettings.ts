import { z } from "zod"
import { useStore } from "zustand"
import { createPersistentStore } from "./persistent-store"
import type { ArrayStoreState } from "./persistent-store"

// Schema for ProjectMetadata with normalization
const ProjectMetadataSchema = z.object({
  path: z.string(),
  cachedBranchPrefix: z.string().optional().transform(val => val === "" ? undefined : val),
  lastSyncTime: z.number().optional(),
  lastBranchCount: z.number().optional(),
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

/** Immutably updates the project with the given path; no-op if it is not in the list. */
export function updateProject(path: string, update: Partial<ProjectMetadata> | ((project: ProjectMetadata) => ProjectMetadata)): void {
  const items = getRecentProjects()
  const index = items.findIndex(it => it.path === path)
  if (index === -1) {
    return
  }

  const current = items[index]!
  const next = typeof update === "function" ? update(current) : { ...current, ...update }
  if (next === current) {
    return
  }
  setRecentProjects(items.with(index, next))
}
