import { lazy, Suspense, useEffect } from "react"
import { CircleXIcon, GitBranchIcon } from "lucide-react"
import pDebounce from "p-debounce"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { ArchivedBranchTableCard } from "@/components/branch/ArchivedBranchTableCard"
import { BranchTableCard } from "@/components/branch/BranchTableCard"
import { GitSetupAlert } from "@/components/repository/GitSetupAlert"
import { LandedCard } from "@/components/repository/LandedCard"
import { ReadyToSyncCard } from "@/components/repository/ReadyToSyncCard"
import { WelcomeCard } from "@/components/repository/WelcomeCard"
import { YourTurnCard } from "@/components/repository/YourTurnCard"
import { useMenuSync } from "@/hooks/useMenuSync"
import { useReviewStatusRefresh } from "@/hooks/useReviewStatusRefresh"
import { useSyncOnOpen } from "@/hooks/useSyncOnOpen"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { useArchivedBranchCount } from "@/stores/archived-branches"
import { syncIfChanged, useBranchSync, useUnassignedCommits } from "@/stores/branch-sync"
import { getSelectedProject, initRepository, useEffectiveBranchPrefix, useRepository, useSelectedProjectField, useSelectedProjectPath } from "@/stores/repository"
import { appSettingsStore } from "@/stores/settings/appSettings"
import { useRecentProjects } from "@/stores/settings/repositorySettings"

/**
 * Window focus handler: with "Auto-sync on Focus" (on by default) and a project selected, syncs when what a sync reads
 * changed since the last one (`syncIfChanged`; during a sync, it checks once after it)
 */
const debouncedAutoSync = pDebounce(async () => {
  if (appSettingsStore.getState().autoSyncOnFocus && getSelectedProject() !== null) {
    await syncIfChanged()
  }
}, 100)

const UnassignedCommitListCard = lazy(() => import("@/components/unassigned/UnassignedCommitListCard").then(it => ({ default: it.UnassignedCommitListCard })))

/** Root of the main window: configuration header, onboarding, errors and the branch tables. */
export function BranchCreator() {
  useEffect(() => {
    initRepository()
  }, [])
  useMenuSync()
  useReviewStatusRefresh()
  useSyncOnOpen()
  useWindowFocus(() => {
    void debouncedAutoSync()
  })

  const isSyncing = useBranchSync(state => state.isSyncing)
  const syncError = useBranchSync(state => state.syncError)
  const syncedRepositoryPath = useBranchSync(state => state.syncedRepositoryPath)
  const branchCount = useBranchSync(state => state.branchOrder.length)
  const unassignedCommits = useUnassignedCommits()
  const unassignedCommitCount = unassignedCommits.length
  const archivedBranchCount = useArchivedBranchCount()

  const pathValidation = useRepository(state => state.pathValidation)
  const isLoadingBranchPrefix = useRepository(state => state.isLoadingBranchPrefix)
  const hasRecentProjects = useRecentProjects(projects => projects.length > 0)
  const selectedPath = useSelectedProjectPath()
  const neverSynced = useSelectedProjectField("lastSyncTime") === undefined
  // a repository counts as selected as soon as a project is chosen
  const hasRepository = selectedPath !== undefined
  const hasSynced = hasRepository && syncedRepositoryPath === selectedPath
  const hasBranchPrefix = useEffectiveBranchPrefix() !== ""
  // git itself is missing or too old: its alert replaces the path and sync errors, which say the same
  const hasGitError = useRepository(state => !!state.gitSetup?.git_error)

  const error = hasGitError || isSyncing ? null : syncError || pathValidation.error
  const hasError = hasGitError || !!error
  const showWelcomeCard = shouldShowWelcomeCard({
    hasRepository,
    hasRecentProjects,
    hasBranchPrefix,
    isLoadingBranchPrefix,
    hasError,
  })
  // a repository synced before syncs when the window opens it (useSyncOnOpen): only the first sync needs the card
  const showReadyToSync = hasRepository && neverSynced && hasBranchPrefix && !isLoadingBranchPrefix && pathValidation.valid && !hasSynced && !hasError

  // a message of several lines (git with its install hint) is a title and its details
  const [errorTitle, ...errorLines] = error?.split("\n") ?? []
  const errorDetails = [errorLines.join("\n"), pathValidation.errorDetails === error ? "" : pathValidation.errorDetails].filter(Boolean).join("\n")
  return (
    <div className="flex flex-col gap-2" data-testid="branch-creator-root">
      <GitSetupAlert />

      {showWelcomeCard && <WelcomeCard hasRepository={hasRepository} hasBranchPrefix={hasBranchPrefix} />}

      {showReadyToSync && <ReadyToSyncCard />}

      <LandedCard />

      {errorTitle ? (
        <Alert variant="destructive" data-testid="error-alert">
          <CircleXIcon />
          <AlertTitle>{errorTitle}</AlertTitle>
          {/* a command error is both the title and the details: said once */}
          {errorDetails && <AlertDescription className="whitespace-pre-line">{errorDetails}</AlertDescription>}
        </Alert>
      ) : (
        hasSynced &&
        branchCount === 0 &&
        unassignedCommitCount === 0 && (
          <Alert data-testid="empty-state-alert">
            <GitBranchIcon />
            <AlertTitle>No active branches</AlertTitle>
            <AlertDescription className="flex flex-col gap-3">
              <p>Commit to your main branch using prefixes like:</p>
              <ul className="list-inside list-disc font-mono">
                <li>(auth) Add login form</li>
                <li>ABC-123 Fix login bug</li>
              </ul>
              <p>Branch Deck will automatically group your commits into virtual branches.</p>
              {archivedBranchCount > 0 && <p className="text-sm text-muted-foreground">Your archived branches are shown below.</p>}
            </AlertDescription>
          </Alert>
        )
      )}

      {/* after the first completed sync: never during Welcome or Ready to Sync */}
      {hasSynced && <YourTurnCard />}

      {unassignedCommitCount > 0 && (
        <Suspense fallback={null}>
          <UnassignedCommitListCard commits={unassignedCommits} />
        </Suspense>
      )}

      <BranchTableCard />

      <ArchivedBranchTableCard />
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
  hasError: boolean
}): boolean {
  // the error alert explains the problem (git missing, a folder that is not a repository); don't show both
  if (state.hasError) {
    return false
  }
  if (!state.hasRepository) {
    return !state.hasRecentProjects
  }
  // the prefix is not known yet
  if (state.isLoadingBranchPrefix) {
    return false
  }
  return !state.hasBranchPrefix
}
