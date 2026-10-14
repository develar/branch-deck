import { lazy, Suspense, useEffect, useState } from "react"
import { CircleXIcon, GitBranchIcon } from "lucide-react"
import pDebounce from "p-debounce"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { ArchivedBranchTableCard } from "@/components/branch/ArchivedBranchTableCard"
import { BranchTableCard } from "@/components/branch/BranchTableCard"
import { ConfigurationHeader } from "@/components/repository/ConfigurationHeader"
import { LandedCard } from "@/components/repository/LandedCard"
import { WelcomeCard } from "@/components/repository/WelcomeCard"
import { useMenuSync } from "@/hooks/useMenuSync"
import { useReviewStatusRefresh } from "@/hooks/useReviewStatusRefresh"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { useArchivedBranchCount } from "@/stores/archived-branches"
import { usePushGuardRequest } from "@/stores/push-guard"
import { branchSyncStore, syncBranches, useBranchSync, useUnassignedCommits } from "@/stores/branch-sync"
import { getSelectedProject, initRepository, useEffectiveBranchPrefix, useRepository, useSelectedProject } from "@/stores/repository"
import { appSettingsStore } from "@/stores/settings/appSettings"
import { useRecentProjects } from "@/stores/settings/repositorySettings"

/** Window focus handler: syncs if "Auto Sync on Focus" is enabled, a project is selected and no sync is running. */
const debouncedAutoSync = pDebounce(async () => {
  if (appSettingsStore.getState().autoSyncOnFocus && getSelectedProject() !== null && !branchSyncStore.getState().isSyncing) {
    await syncBranches()
  }
}, 100)

const PushGuardDialog = lazy(() => import("@/components/branch/PushGuardDialog").then(it => ({ default: it.PushGuardDialog })))
const UnassignedCommitListCard = lazy(() => import("@/components/unassigned/UnassignedCommitListCard").then(it => ({ default: it.UnassignedCommitListCard })))

/** Root of the main window: configuration header, onboarding, errors and the branch tables. */
export function BranchCreator() {
  useEffect(() => {
    initRepository()
  }, [])
  useMenuSync()
  useReviewStatusRefresh()
  useWindowFocus(() => {
    void debouncedAutoSync()
  })

  const isSyncing = useBranchSync(state => state.isSyncing)
  const syncError = useBranchSync(state => state.syncError)
  const hasCompletedSync = useBranchSync(state => state.hasCompletedSync)
  const branchCount = useBranchSync(state => state.branchOrder.length)
  const unassignedCommits = useUnassignedCommits()
  const unassignedCommitCount = unassignedCommits.length
  const archivedBranchCount = useArchivedBranchCount()
  // mounted on the first question and kept, so the dialog animates and returns the focus when it closes
  const hasPushGuardRequest = usePushGuardRequest() !== null
  const [isPushGuardMounted, setPushGuardMounted] = useState(false)
  if (hasPushGuardRequest && !isPushGuardMounted) {
    setPushGuardMounted(true)
  }

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
      <ConfigurationHeader />

      {showWelcomeCard && <WelcomeCard hasRepository={hasRepository} hasBranchPrefix={hasBranchPrefix} />}

      <LandedCard />

      {errorTitle && !isSyncing
        ? (
            <Alert variant="destructive" data-testid="error-alert">
              <CircleXIcon />
              <AlertTitle>{errorTitle}</AlertTitle>
              {/* a command error is both the title and the details: said once */}
              {pathValidation.errorDetails && pathValidation.errorDetails !== errorTitle && <AlertDescription>{pathValidation.errorDetails}</AlertDescription>}
            </Alert>
          )
        : hasCompletedSync && branchCount === 0 && unassignedCommitCount === 0 && (
          <Alert data-testid="empty-state-alert">
            <GitBranchIcon />
            <AlertTitle>No active branches</AlertTitle>
            <AlertDescription className="flex flex-col gap-3">
              <p>Commit to your main branch using prefixes like:</p>
              <ul className="list-inside list-disc font-mono">
                <li>(auth) Add login form</li>
                <li>YT-123 Fix bug</li>
              </ul>
              <p>Branch Deck will automatically group your commits into virtual branches.</p>
              {archivedBranchCount > 0 && (
                <p className="text-sm text-muted-foreground">Your archived branches are shown below.</p>
              )}
            </AlertDescription>
          </Alert>
        )}

      {unassignedCommitCount > 0 && (
        <Suspense fallback={null}>
          <UnassignedCommitListCard commits={unassignedCommits} />
        </Suspense>
      )}

      <BranchTableCard />

      <ArchivedBranchTableCard />

      {isPushGuardMounted && (
        <Suspense fallback={null}>
          <PushGuardDialog />
        </Suspense>
      )}
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
