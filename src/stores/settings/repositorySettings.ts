import { z } from "zod"
import { useStore } from "zustand"
import type { IssueNavigationConfig } from "@/lib/bindings"
import { stableStringify } from "@/lib/stable-stringify"
import { createPersistentStore } from "./persistent-store"
import type { ArrayStoreState } from "./persistent-store"

/** The issue links of the repository's `.idea/vcs.xml`, as the last sync read them */
const IssueNavigationConfigSchema = z.object({
  links: z.array(z.object({ issueRegexp: z.string(), linkRegexp: z.string() })),
}) satisfies z.ZodType<IssueNavigationConfig>

export const ProjectMetadataSchema = z.object({
  path: z.string(),
  /** The user collapsed the "Your Turn" card of the repository (open by default) */
  reviewRequestsCollapsed: z.boolean().optional(),
  /** The user collapsed the Parked card of the repository (open by default) */
  parkedCollapsed: z.boolean().optional(),
  /** The stack roots the user collapsed in the branch table (expanded by default, not stored) */
  collapsedStacks: z.array(z.string()).optional(),
  /** A malformed value is dropped, never the recent projects */
  issueNavigationConfig: IssueNavigationConfigSchema.optional().catch(undefined),
})

const RepositorySettingsSchema = z.array(ProjectMetadataSchema).default([])

export type ProjectMetadata = z.infer<typeof ProjectMetadataSchema>

/**
 * Recent projects, most recently selected first. `items[0]` is the selected project. What each sync records of a
 * repository is not here but in `repositoryState` (`@/stores/settings/repositoryState`), which a sync changes without
 * rewriting the user's settings.
 */
export const repositorySettingsStore = createPersistentStore("recentProjects", { schema: RepositorySettingsSchema, isArray: true, isMainOnly: false })

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

/** The collapsed stacks of the repository, by root; none is stored as no list */
export function setCollapsedStacks(path: string, roots: readonly string[]): void {
  updateProject(path, project => {
    const current = project.collapsedStacks ?? []
    const same = current.length === roots.length && current.every((it, index) => it === roots[index])
    return same ? project : { ...project, collapsedStacks: roots.length === 0 ? undefined : [...roots] }
  })
}

/** Collapses or opens the Parked card of the repository; open is the default and is not stored */
export function setParkedCollapsed(path: string, collapsed: boolean): void {
  updateProject(path, project => ((project.parkedCollapsed === true) === collapsed ? project : { ...project, parkedCollapsed: collapsed ? true : undefined }))
}

/** Collapses or opens the "Your Turn" card of the repository; open is the default and is not stored */
export function setReviewRequestsCollapsed(path: string, collapsed: boolean): void {
  updateProject(path, project => ((project.reviewRequestsCollapsed === true) === collapsed ? project : { ...project, reviewRequestsCollapsed: collapsed ? true : undefined }))
}
