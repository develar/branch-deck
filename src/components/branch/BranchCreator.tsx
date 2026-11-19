import { useEffect } from "react"
import { CircleXIcon, GitBranchIcon, HourglassIcon, TriangleAlertIcon } from "lucide-react"
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
import { useOnboardingReady } from "@/stores/onboarding"
import { useEffectiveBranchPrefix, useRepository, useSelectedProjectPath, useSessionFailure } from "@/stores/repository"
import { useAppSettings } from "@/stores/settings/appSettings"
import { useRecentProjects } from "@/stores/settings/repositorySettings"
import { useRowsGeneration } from "@/stores/view-rows"

/**
 * The branches view of the main window: onboarding, errors and the branch tables. What keeps the repository current (its
 * open, the sync on open, the review status) lives in `IndexPage`, so it goes on while another view shows.
 */
export function BranchCreator() {
  const isSyncing = useBranchSync(state => state.isSyncing)
  // the failure of the last sync, kept while the next one runs (`syncFailure`): the alert stays where it is
  const syncFailure = useBranchSync(state => state.syncFailure?.message ?? null)
  // a sync git could not write the notes of without the user's name: the git alert says it, with the commands
  const syncNeedsIdentity = useBranchSync(state => state.syncFailure?.code === "no_identity")
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
  // why the session of the repository serves no window: no prefix (the welcome card), no git (the git alert), a
  // repository git cannot read (the error alert of the path)
  const sessionFailure = useSessionFailure()
  const onboardingReady = useOnboardingReady()
  const firstRun = useAppSettings(state => state.onboardingStatus === "inProgress")
  const hasRecentProjects = useRecentProjects(projects => projects.length > 0)
  const selectedPath = useSelectedProjectPath()
  // the session decides whether the repository was synced before, by the app or by `brd` (`RepositoryView.synced`)
  const neverSynced = !useBranchSync(state => state.syncedBefore)
  // the session has not answered yet: no view and no failure, so the page knows neither the card nor the rows
  const awaitsSession = useBranchSync(state => state.viewVersion === null)
  // the view of another session of the repository: the cards mount anew, so nothing they held of the rows before stays
  const rowsGeneration = useRowsGeneration()
  // a repository counts as selected as soon as a project is chosen
  const hasRepository = selectedPath !== undefined
  const hasSynced = hasRepository && syncedRepositoryPath === selectedPath
  const hasBranchPrefix = useEffectiveBranchPrefix() !== ""
  const noBranchPrefix = sessionFailure?.code === "no_branch_prefix"
  // git itself is missing or too old (the git alert), or cannot name the user: its alert replaces the errors that say the same
  const hasGitError = sessionFailure?.code === "git_not_found" || syncNeedsIdentity

  // a repository git cannot read is the error of the path (`pathValidation`, which the failed subscription sets)
  const error = hasGitError ? null : syncFailure || pathValidation.error
  const hasError = hasGitError || !!error
  // The saved first-time walkthrough also stays while the chosen repository session checks its prefix.
  const showWelcomeCard = onboardingReady && !hasError && (hasRepository ? noBranchPrefix || firstRun : !hasRecentProjects)
  // a repository synced before syncs when its session opens (the session's own sync): only the first sync needs the card
  const showReadyToSync = !showWelcomeCard && hasRepository && neverSynced && hasBranchPrefix && pathValidation.valid && !hasSynced && !hasError

  // the window shows on its first frame with content: the first rows of a sync, or a state without rows (no repository,
  // Welcome, Ready to Sync, an error, a sync that ended without rows)
  const hasContent =
    (!hasRepository && onboardingReady) || showWelcomeCard || showReadyToSync || hasError || hasSynced || hasSyncEnded || branchCount > 0 || unassignedCommitCount > 0
  useEffect(() => {
    if (hasContent) {
      markWindowContentReady()
    }
  }, [hasContent])
  // a sync git refused (a rebase in progress, a detached HEAD) says why where there is no table to show; one that no work
  // of the user ends (a baseline unrelated to the branch) says it above the table of the sync before too
  const showDeferred = syncDeferred !== null && !isSyncing && !hasError && (syncDeferred.tone === "warning" || (branchCount === 0 && unassignedCommitCount === 0))
  // until the session of the repository answers, the table loads where it will be (the shell of the window has it); a
  // repository synced before syncs when the window opens it: until its rows come, the table loads, also during a sync
  // after one that failed or waited for git, and during the first one of another session (a prefix changed in a
  // terminal) after the window saw a sync end
  const awaitsAnswer = awaitsSession && sessionFailure === null
  const isLoadingRows = hasRepository && (awaitsAnswer || !neverSynced) && !hasSynced && !hasError && (isSyncing || !hasContent) && branchCount === 0 && unassignedCommitCount === 0

  // a message of several lines (git with its install hint) is a title and its details
  const [errorTitle, ...errorLines] = error?.split("\n") ?? []
  const errorDetails = [errorLines.join("\n"), error === pathValidation.error && pathValidation.errorDetails !== error ? pathValidation.errorDetails : ""]
    .filter(Boolean)
    .join("\n")
  return (
    <div key={rowsGeneration} className="flex flex-col gap-2" data-testid="branch-creator-root">
      <GitSetupAlert />

      {showWelcomeCard && <WelcomeCard repositoryPath={selectedPath} firstRun={firstRun} needsPrefix={noBranchPrefix} />}

      {showReadyToSync && <ReadyToSyncCard />}

      <LandedCard />

      {showDeferred && (
        <Alert variant={syncDeferred.tone} data-testid="sync-deferred-alert">
          {syncDeferred.tone === "warning" ? <TriangleAlertIcon /> : <HourglassIcon />}
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
