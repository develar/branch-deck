import { useEffectiveBranchPrefix, useIsSubscribing, useRepository, useSelectedProjectPath, useSessionFailure } from "@/stores/repository"

/**
 * Whether Sync can run, and why not: an invalid repository (also one its subscription found git cannot read), no git,
 * a missing branch prefix; for the button and the menus
 */
export function useSyncAvailability(): { canSync: boolean; reason: string | null } {
  const pathValidation = useRepository(state => state.pathValidation)
  const failure = useSessionFailure()
  const hasSelectedProject = useSelectedProjectPath() !== undefined
  const prefix = useEffectiveBranchPrefix()
  // not while the subscription has not answered: the button would flicker; the sync resolves the prefix in Rust itself
  const isSubscribing = useIsSubscribing()
  const canSync = hasSelectedProject && pathValidation.valid && failure === null && (prefix !== "" || isSubscribing)
  if (canSync || !hasSelectedProject) {
    return { canSync, reason: null }
  }
  // git with its install hint says its first line
  const noGit = failure?.code === "git_not_found" ? (failure.message.split("\n")[0] ?? null) : null
  return { canSync, reason: pathValidation.error ?? noGit ?? "Set the branch prefix first" }
}
