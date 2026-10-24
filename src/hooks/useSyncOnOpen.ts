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
 * (`src/model/store.ts`). A change of the branch prefix after it loaded is another session too.
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
  // a failed sync isn't repeated until the window opens the repository again; ⌘R and Sync retry it
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
    ensureSession(path)
    if (isLoadingBranchPrefix) {
      return
    }
    if (knownPrefix.current !== null && knownPrefix.current !== branchPrefix) {
      // another prefix names the branches: the session of the repository under it
      openSession(path)
    }
    knownPrefix.current = branchPrefix
    if (branchPrefix === "" || !wasSynced || hasSyncedOnOpen.current) {
      return
    }
    hasSyncedOnOpen.current = true
    // the first sync of a repository never synced is what sets its lastSyncTime: the page already shows it
    if (branchSyncStore.getState().syncedRepositoryPath !== path) {
      void syncBranches("open")
    }
  }, [path, wasSynced, isLoadingBranchPrefix, isValid, hasGitError, branchPrefix])
}
