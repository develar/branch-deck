import { useEffectiveBranchPrefix, useRepository, useSelectedProjectPath } from "@/stores/repository"

/** Whether Sync can run, and why not: an invalid repository or a missing branch prefix; for the button and the menus */
export function useSyncAvailability(): { canSync: boolean; reason: string | null } {
  const pathValidation = useRepository(state => state.pathValidation)
  const hasSelectedProject = useSelectedProjectPath() !== undefined
  // not while the prefix loads: the button would flicker; sync waits for the load
  const effectiveBranchPrefix = useEffectiveBranchPrefix()
  const isLoadingBranchPrefix = useRepository(state => state.isLoadingBranchPrefix)
  const hasBranchPrefix = effectiveBranchPrefix !== "" || isLoadingBranchPrefix
  const canSync = hasSelectedProject && pathValidation.valid && hasBranchPrefix
  const reason = !canSync && pathValidation.error ? pathValidation.error : hasSelectedProject && !hasBranchPrefix ? "Set the branch prefix first" : null
  return { canSync, reason }
}
