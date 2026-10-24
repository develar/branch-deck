import { z } from "zod"
import { useStore } from "zustand"
import type { UpdateTimings } from "@/lib/update-progress"
import { createPersistentStore } from "./persistent-store"

/*
 * What the syncs and the updates record of each repository, by its path: the key `repositoryState`, which the writer of
 * the settings keeps in `repository-state.json` beside `settings.json` (`app_settings::file`), so a sync never rewrites
 * the file of the user's settings that `bd` reads. Only the main window has it; a settings file of before kept these
 * fields in each recent project, and the writer moved them here when it loaded it.
 */

/** The durations of the steps of Update Project and of a sync (ms); a malformed list is dropped, never the repository */
const UpdateTimingsSchema = z
  .object({
    fetch: z.array(z.number()).optional(),
    check: z.array(z.number()).optional(),
    apply: z.array(z.number()).optional(),
    sync: z.array(z.number()).optional(),
  })
  .optional()
  .catch(undefined) satisfies z.ZodType<UpdateTimings | undefined>

/** A malformed field is dropped, never the repository */
const RepositoryStateEntrySchema = z.object({
  /** When the last sync of the repository ended (ms) */
  lastSyncTime: z.number().optional().catch(undefined),
  /** How many branches the last sync had */
  lastBranchCount: z.number().optional().catch(undefined),
  /** The last durations of each step of Update Project and of a sync, the shares of its progress (`update-progress.ts`) */
  updateTimings: UpdateTimingsSchema,
  /** When Branch Deck last fetched the baseline (ms); a fetch elsewhere shows through FETCH_HEAD */
  lastFetchTime: z.number().optional().catch(undefined),
  /**
   * The Landed island hides the branches that landed up to this time (`landedAt`, Unix seconds): a sync hid them after
   * the user had seen them. Their chips stay until Dismiss.
   */
  landedHiddenUntil: z.number().optional().catch(undefined),
  /**
   * The layout of the branch view once the facts of the last start were in (`useSettledLayout`): the widths of the
   * tracks of the branch columns and the height of Your Turn (px, 0 without it); the next start and a switch back
   * reserve them until their own facts are in
   */
  settledLayout: z
    .object({ tracks: z.record(z.string(), z.number()), yourTurn: z.number() })
    .optional()
    .catch(undefined),
})

/** A malformed repository is dropped, never the others */
const RepositoryStateSchema = z.record(z.string(), RepositoryStateEntrySchema.catch({}))

export type RepositoryStateEntry = z.infer<typeof RepositoryStateEntrySchema>

export const repositoryStateStore = createPersistentStore("repositoryState", { schema: RepositoryStateSchema, isArray: false, isMainOnly: true })

/** What the syncs recorded of the repository at `path`; nothing for one never synced */
export function getRepositoryState(path: string): RepositoryStateEntry | undefined {
  return repositoryStateStore.getState()[path]
}

/** One field of the repository at `path`; re-renders only when it changes */
export function useRepositoryStateField<K extends keyof RepositoryStateEntry>(path: string | undefined, field: K): RepositoryStateEntry[K] | undefined {
  return useStore(repositoryStateStore, state => (path === undefined ? undefined : state[path]?.[field]))
}

/** Changes what is recorded of the repository at `path`; the writer gets only that repository (a patch of one field) */
export function updateRepositoryState(path: string, update: (state: RepositoryStateEntry) => RepositoryStateEntry): void {
  const states = repositoryStateStore.getState()
  repositoryStateStore.setState({ ...states, [path]: update(states[path] ?? {}) }, true)
}
