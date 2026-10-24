import { useEffect, useRef } from "react"
import { closeSession, ensureSession, openSession } from "@/model/store"
import { branchSyncStore, syncBranches } from "@/stores/branch-sync"
import { useEffectiveBranchPrefix, useIsLoadingBranchPrefix, useRepository, useSelectedProjectPath, useSelectedRepositoryState } from "@/stores/repository"

/**
 * Subscribes the window to the session of the selected repository and syncs a repository synced before when the
 * window opens it, at start and on a switch, whether or not Auto-sync on Focus is on: the sync is local and leaves the
 * working tree alone, so the branches and Update show without a click. A repository never synced waits for the Sync of
 * the Ready to Sync card. The window subscribes at once, without waiting for the prefix the page shows: the session is
 * keyed by the prefix Rust resolves. A switch first forgets the branches of the previous repository, at once: the
 * session of the new one is another session, whose patches never mix with the late ones of the old
 * (`src/model/store.ts`). A change of the branch prefix after it loaded (a `git config` in a terminal, found on a
 * focus; a write of Settings) is another session too, which the window opens and syncs as it opens a repository: the
 * table is never the empty one of a session that no sync built. Without a prefix the window holds no session (one is
 * refused as `no_branch_prefix`): the welcome card asks for the prefix.
 */
export function useSyncOnOpen(): void {
  const path = useSelectedProjectPath() ?? null
  const wasSynced = useSelectedRepositoryState("lastSyncTime") !== undefined
  const isLoadingBranchPrefix = useIsLoadingBranchPrefix()
  const isValid = useRepository(state => state.pathValidation.valid)
  const hasGitError = useRepository(state => !!state.gitSetup?.git_error)
  const branchPrefix = useEffectiveBranchPrefix()
  // undefined until the first render: the start has nothing to forget
  const openedPath = useRef<string | null | undefined>(undefined)
  // the prefix the page loaded for the session the window holds; null before it loaded
  const knownPrefix = useRef<string | null>(null)
  // a failed sync isn't repeated until the window opens the repository (or its session under another prefix) again; ⌘R
  // and Sync retry it
  const hasSyncedOnOpen = useRef(false)

  useEffect(() => {
    if (openedPath.current !== path) {
      if (openedPath.current !== undefined) {
        closeSession()
      }
      openedPath.current = path
      knownPrefix.current = null
      hasSyncedOnOpen.current = false
    }
    if (path === null || !isValid || hasGitError) {
      return
    }
    if (isLoadingBranchPrefix) {
      ensureSession(path)
      return
    }
    const prefixChanged = knownPrefix.current !== null && knownPrefix.current !== branchPrefix
    knownPrefix.current = branchPrefix
    if (branchPrefix === "") {
      if (prefixChanged) {
        closeSession()
      }
      return
    }
    if (prefixChanged) {
      // another prefix names the branches: the session of the repository under it, synced as an opened one
      openSession(path)
      hasSyncedOnOpen.current = false
    } else {
      ensureSession(path)
    }
    if (!wasSynced || hasSyncedOnOpen.current) {
      return
    }
    hasSyncedOnOpen.current = true
    // the first sync of a repository never synced is what sets its lastSyncTime: the page already shows it
    if (branchSyncStore.getState().syncedRepositoryPath !== path) {
      void syncBranches("open")
    }
  }, [path, wasSynced, isLoadingBranchPrefix, isValid, hasGitError, branchPrefix])
}
