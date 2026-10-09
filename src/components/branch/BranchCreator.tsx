import { lazy, Suspense, useEffect } from "react"
import { CircleXIcon, GitBranchIcon } from "lucide-react"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { ConfigurationHeader } from "@/components/repository/ConfigurationHeader"
import { initRepository, useEffectiveBranchPrefix, useRepository, useSelectedProject } from "@/stores/repository"
import { useRecentProjects } from "@/stores/settings/repositorySettings"

const WelcomeCard = lazy(() => import("@/components/repository/WelcomeCard").then(it => ({ default: it.WelcomeCard })))

/** Subset of the branch-sync state BranchCreator renders. */
interface BranchSyncView {
  isSyncing: boolean
  syncError: string | null
  hasCompletedSync: boolean
  branchCount: number
  unassignedCommitCount: number
  archivedBranchCount: number
  syncBranches: () => void
}

// TODO(Phase 5): replace with selectors on the branch-sync store (src/stores/branch-sync.ts)
const placeholderBranchSync: BranchSyncView = {
  isSyncing: false,
  syncError: null,
  hasCompletedSync: false,
  branchCount: 0,
  unassignedCommitCount: 0,
  archivedBranchCount: 0,
  syncBranches: () => {},
}

/** Root of the main window: configuration header, onboarding, errors and the branch tables. */
export function BranchCreator() {
  useEffect(() => {
    initRepository()
  }, [])
  // TODO(Phase 8): model state for AI features (provideModelState)
  // TODO(Phase 9): menu checkbox sync (useMenuSync) and debounced auto-sync on window focus

  const { isSyncing, syncError, hasCompletedSync, branchCount, unassignedCommitCount, archivedBranchCount, syncBranches } = placeholderBranchSync

  const pathValidation = useRepository(state => state.pathValidation)
  const isLoadingBranchPrefix = useRepository(state => state.isLoadingBranchPrefix)
  const hasRecentProjects = useRecentProjects(projects => projects.length > 0)
  // a repository counts as selected as soon as a project is chosen
  const hasRepository = useSelectedProject() !== null
  const hasBranchPrefix = useEffectiveBranchPrefix() !== ""

  const showWelcomeCard = shouldShowWelcomeCard({
    hasRepository,
    hasRecentProjects,
    hasBranchPrefix,
    isLoadingBranchPrefix,
    hasPathError: !!pathValidation.error && !isSyncing,
  })

  const errorTitle = syncError || pathValidation.error
  return (
    <div className="flex flex-col gap-6" data-testid="branch-creator-root">
      <ConfigurationHeader isSyncing={isSyncing} syncError={syncError} branchCount={branchCount} onSync={syncBranches} />

      {showWelcomeCard && (
        <Suspense fallback={null}>
          <WelcomeCard hasRepository={hasRepository} hasBranchPrefix={hasBranchPrefix} />
        </Suspense>
      )}

      {errorTitle && !isSyncing
        ? (
            <Alert variant="destructive" className="border-transparent bg-destructive/10" data-testid="error-alert">
              <CircleXIcon />
              <AlertTitle>{errorTitle}</AlertTitle>
              {pathValidation.errorDetails && <AlertDescription>{pathValidation.errorDetails}</AlertDescription>}
            </Alert>
          )
        : hasCompletedSync && branchCount === 0 && unassignedCommitCount === 0 && (
          <Alert className="bg-muted/50" data-testid="empty-state-alert">
            <GitBranchIcon />
            <AlertTitle>No active branches</AlertTitle>
            <AlertDescription className="flex flex-col gap-3">
              <p>Commit to your main branch using prefixes like:</p>
              <ul className="list-inside list-disc font-mono">
                <li>(auth) Add login form</li>
                <li>YT-123 Fix bug</li>
              </ul>
              <p>BranchDeck will automatically group your commits into virtual branches.</p>
              {archivedBranchCount > 0 && (
                <p className="text-sm text-muted-foreground">Your archived branches are shown below.</p>
              )}
            </AlertDescription>
          </Alert>
        )}

      {/* TODO Phase 5: <UnassignedCommitListCard /> (lazy) when there are unassigned commits */}
      {/* TODO Phase 5: <BranchTableCard /> */}
      {/* TODO Phase 5: <ArchivedBranchTableCard /> */}
    </div>
  )
}

/**
 * Shown on first run (no repository selected and no recent projects),
 * or when a repository is selected but no branch prefix is configured (guides the user to step 2).
 */
function shouldShowWelcomeCard(state: {
  hasRepository: boolean
  hasRecentProjects: boolean
  hasBranchPrefix: boolean
  isLoadingBranchPrefix: boolean
  hasPathError: boolean
}): boolean {
  if (!state.hasRepository) {
    return !state.hasRecentProjects
  }
  // the prefix is not known yet
  if (state.isLoadingBranchPrefix) {
    return false
  }
  // the error alert explains the problem; don't show both
  if (state.hasPathError) {
    return false
  }
  return !state.hasBranchPrefix
}
