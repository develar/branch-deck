import { useEffect, useRef } from "react"
import { appPrefixOf } from "@/lib/branch-prefix"
import { closeSession, ensureSession, resubscribeSession } from "@/model/store"
import { useRepository, useSelectedProjectPath } from "@/stores/repository"
import { onWriterChange, snapshotValueOf } from "@/stores/settings/persistent-store"

/**
 * Subscribes the window to the session of the selected repository when the window opens it, at start and on a switch.
 * The page reads nothing before: the subscription is the first command (the warm start has made it already,
 * `adoptCachedModel`), and its answer says what the page shows: the view of the session, named by the prefix Rust
 * resolved for it (`TableHeader.prefix`), or why there is none by its code (no branch prefix: the welcome card; no git:
 * the git alert; a repository git cannot read: the error alert). The session syncs a repository synced before as it
 * opens and as a window comes back to it (`synced_before` of its host, the `lastSyncTime` the backend's writer keeps),
 * whether or not Auto-sync on Focus is on, so the page asks for no sync of its own; a repository never synced waits
 * for the Sync of the Ready to Sync card. A switch first forgets the branches of the previous repository, at once: the
 * session of the new one is another session, whose patches never mix with the late ones of the old
 * (`src/model/store.ts`). A change of the app's prefix (from any window, once the writer has it) subscribes again
 * (`resubscribeSession`): the prefix that names the branches may be another one, which is another session, and Rust
 * resolves it; so does a write of Settings → Repository (`useRepositoryConfigChanges`) and a `git config` in a terminal
 * that a sync or the check of a focus found (`prefix_changed`, `src/model/store.ts`).
 */
export function useSyncOnOpen(): void {
  const path = useSelectedProjectPath() ?? null
  const isValid = useRepository(state => state.pathValidation.valid)
  // undefined until the first render: the start has nothing to forget
  const openedPath = useRef<string | null | undefined>(undefined)

  useEffect(() => {
    if (openedPath.current !== path) {
      if (openedPath.current !== undefined) {
        closeSession()
      }
      openedPath.current = path
    }
    if (path !== null && isValid) {
      ensureSession(path)
    }
  }, [path, isValid])

  useEffect(() => {
    // the app's prefix as the writer has it: a change of it, from any window, may name the branches anew
    let appPrefix = appPrefixOf(snapshotValueOf("appSettings"))
    return onWriterChange("appSettings", value => {
      const prefix = appPrefixOf(value)
      if (prefix !== appPrefix) {
        appPrefix = prefix
        void resubscribeSession()
      }
    })
  }, [])
}
