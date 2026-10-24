import { useEffect, useRef } from "react"
import { branchSyncStore, resetBranchSync, syncBranches, useBranchSync } from "@/stores/branch-sync"
import { useEffectiveBranchPrefix, useIsLoadingBranchPrefix, useRepository, useSelectedProjectField, useSelectedProjectPath } from "@/stores/repository"

/**
 * Syncs a repository synced before when the window opens it, at start and on a switch, whether or not Auto-sync on
 * Focus is on: the sync is local and leaves the working tree alone, so the branches and Update show without a click.
 * A repository never synced waits for the Sync of the Ready to Sync card. A switch first forgets the branches of the
 * previous repository, once its running sync is done, so they never show under the new name.
 */
export function useSyncOnOpen(): void {
  const path = useSelectedProjectPath() ?? null
  const wasSynced = useSelectedProjectField("lastSyncTime") !== undefined
  const isSyncing = useBranchSync(state => state.isSyncing)
  const isLoadingBranchPrefix = useIsLoadingBranchPrefix()
  const isValid = useRepository(state => state.pathValidation.valid)
  const hasGitError = useRepository(state => !!state.gitSetup?.git_error)
  const hasBranchPrefix = useEffectiveBranchPrefix() !== ""
  // undefined until the first render with a settled sync: the start has nothing to forget
  const openedPath = useRef<string | null | undefined>(undefined)
  // a failed sync isn't repeated until the window opens the repository again; ⌘R and Sync retry it
  const hasSyncedOnOpen = useRef(false)

  useEffect(() => {
    if (isSyncing) {
      return
    }
    if (openedPath.current !== path) {
      if (openedPath.current !== undefined) {
        resetBranchSync()
      }
      openedPath.current = path
      hasSyncedOnOpen.current = false
    }
    if (path === null || !wasSynced || hasSyncedOnOpen.current || isLoadingBranchPrefix || !isValid || hasGitError || !hasBranchPrefix) {
      return
    }
    // the first sync of a repository never synced is what sets its lastSyncTime: the page already shows it
    if (branchSyncStore.getState().syncedRepositoryPath === path) {
      hasSyncedOnOpen.current = true
      return
    }
    hasSyncedOnOpen.current = true
    void syncBranches()
  }, [path, wasSynced, isSyncing, isLoadingBranchPrefix, isValid, hasGitError, hasBranchPrefix])
}
