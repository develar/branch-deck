import { useEffect } from "react"
import { CircleXIcon, GitBranchIcon, HourglassIcon } from "lucide-react"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { ArchivedBranchTableCard } from "@/components/branch/ArchivedBranchTableCard"
import { BranchTableCard } from "@/components/branch/BranchTableCard"
import { UnappliedBranchTableCard } from "@/components/branch/UnappliedBranchTableCard"
import { AgentsHint } from "@/components/repository/AgentsHint"
import { GitSetupAlert } from "@/components/repository/GitSetupAlert"
import { LandedCard } from "@/components/repository/LandedCard"
import { ReadyToSyncCard } from "@/components/repository/ReadyToSyncCard"
import { WelcomeCard } from "@/components/repository/WelcomeCard"
import { YourTurnSlot } from "@/components/repository/YourTurnCard"
import { InlineCode } from "@/components/shared/InlineCode"
import { UnassignedCommitListCard } from "@/components/unassigned/UnassignedCommitListCard"
import { markWindowContentReady } from "@/lib/sub-window"
import { useArchivedBranchCount } from "@/stores/archived-branches"
import { useBranchSync, useUnassignedCommits } from "@/stores/branch-sync"
import { useEffectiveBranchPrefix, useIsLoadingBranchPrefix, useRepository, useSelectedProjectPath, useSelectedRepositoryState } from "@/stores/repository"
import { useRecentProjects } from "@/stores/settings/repositorySettings"

/**
 * The branches view of the main window: onboarding, errors and the branch tables. What keeps the repository current (its
 * open, the sync on open, the review status) lives in `IndexPage`, so it goes on while another view shows.
 */
export function BranchCreator() {
  const isSyncing = useBranchSync(state => state.isSyncing)
  const syncError = useBranchSync(state => state.syncError)
  const syncDeferred = useBranchSync(state => state.syncDeferred)
  const baselineBranch = useBranchSync(state => state.baselineBranch)
  const syncedRepositoryPath = useBranchSync(state => state.syncedRepositoryPath)
  // a sync of the window ended: also one that git deferred (a rebase in progress), which leaves no rows
  const hasSyncEnded = useBranchSync(state => state.syncCount > 0)
  const branchCount = useBranchSync(state => state.branchOrder.length)
  const unassignedCommits = useUnassignedCommits()
  const unassignedCommitCount = unassignedCommits.length
  const archivedBranchCount = useArchivedBranchCount()

  const pathValidation = useRepository(state => state.pathValidation)
  const isLoadingBranchPrefix = useIsLoadingBranchPrefix()
  const hasRecentProjects = useRecentProjects(projects => projects.length > 0)
  const selectedPath = useSelectedProjectPath()
  const neverSynced = useSelectedRepositoryState("lastSyncTime") === undefined
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
  // a repository synced before syncs when its session opens (the session's own sync): only the first sync needs the card
  const showReadyToSync = hasRepository && neverSynced && hasBranchPrefix && !isLoadingBranchPrefix && pathValidation.valid && !hasSynced && !hasError

  // the window shows on its first frame with content: the first rows of a sync, or a state without rows (no repository,
  // Welcome, Ready to Sync, an error, a sync that ended without rows)
  const hasContent = !hasRepository || showWelcomeCard || showReadyToSync || hasError || hasSynced || hasSyncEnded || branchCount > 0 || unassignedCommitCount > 0
  useEffect(() => {
    if (hasContent) {
      markWindowContentReady()
    }
  }, [hasContent])
  // a sync git refused (a rebase in progress, a detached HEAD) says why where there is no table to show
  const showDeferred = syncDeferred !== null && !isSyncing && !hasError && branchCount === 0 && unassignedCommitCount === 0
  // a repository synced before syncs when the window opens it: until its rows come, the table loads where it will be,
  // also during a sync after one that failed or waited for git, and during the first one of another session (a prefix
  // changed in a terminal) after the window saw a sync end
  const isLoadingRows = hasRepository && !neverSynced && !hasSynced && !hasError && (isSyncing || !hasContent) && branchCount === 0 && unassignedCommitCount === 0

  // a message of several lines (git with its install hint) is a title and its details
  const [errorTitle, ...errorLines] = error?.split("\n") ?? []
  const errorDetails = [errorLines.join("\n"), pathValidation.errorDetails === error ? "" : pathValidation.errorDetails].filter(Boolean).join("\n")
  return (
    <div className="flex flex-col gap-2" data-testid="branch-creator-root">
      <GitSetupAlert />

      {showWelcomeCard && <WelcomeCard hasRepository={hasRepository} hasBranchPrefix={hasBranchPrefix} />}

      {showReadyToSync && <ReadyToSyncCard />}

      <LandedCard />

      {showDeferred && (
        <Alert variant="info" data-testid="sync-deferred-alert">
          <HourglassIcon />
          <AlertTitle>{syncDeferred.title}</AlertTitle>
          <AlertDescription>{syncDeferred.description}</AlertDescription>
        </Alert>
      )}

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
              <p>
                {baselineBranch !== null && (
                  <span data-testid="empty-state-baseline">
                    {"No commits ahead of "}
                    <InlineCode>{baselineBranch}</InlineCode>
                    {". "}
                  </span>
                )}
                Commit to your branch using prefixes like:
              </p>
              <ul className="list-inside list-disc font-mono">
                <li>(auth) Add login form</li>
                <li>ABC-123 Fix login bug</li>
              </ul>
              <p>Branch Deck will automatically group your commits into virtual branches.</p>
              <AgentsHint />
              {archivedBranchCount > 0 && <p className="text-sm text-muted-foreground">Your archived branches are shown below.</p>}
            </AlertDescription>
          </Alert>
        )
      )}

      {/*
        after the first completed sync: never during Welcome or Ready to Sync; until the facts of the remote are in, its
        place as it was the last time
      */}
      <YourTurnSlot shown={hasSynced} reserve={hasRepository && !showWelcomeCard && !showReadyToSync && !hasError} />

      {/* with the rows, never after them: a card that came later moved the table away from the pointer */}
      {unassignedCommitCount > 0 && <UnassignedCommitListCard commits={unassignedCommits} />}

      <BranchTableCard isLoadingRows={isLoadingRows} />

      <UnappliedBranchTableCard />

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
