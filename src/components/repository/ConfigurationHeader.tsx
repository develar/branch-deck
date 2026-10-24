import { lazy, Suspense, useState } from "react"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { PatronusChip } from "@/components/branch/PatronusChip"
import { PullRequestChip } from "@/components/branch/PullRequestChip"
import { ActivityButton } from "@/components/repository/ActivityButton"
import { LandingIndicator } from "@/components/repository/LandingIndicator"
import { MainMenuButton } from "@/components/repository/MainMenuButton"
import { MainViewSwitcher } from "@/components/repository/MainViewSwitcher"
import { PatronusStatusLine } from "@/components/repository/PatronusStatusLine"
import { PullRequestStatusLine } from "@/components/repository/PullRequestStatusLine"
import { RepositorySelector } from "@/components/repository/RepositorySelector"
import { SettingsButton } from "@/components/repository/SettingsButton"
import { SyncButton } from "@/components/repository/SyncButton"
import { UpdateProjectButton, UpdateStateLine } from "@/components/repository/UpdateProjectButton"
import { RelativeTime } from "@/components/shared/RelativeTime"
import { WindowTitleBar } from "@/components/shared/WindowTitleBar"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useHasFilterBar } from "@/hooks/useBranchFilter"
import { hasCustomTitleBar } from "@/lib/title-bar"
import { useBranchSync } from "@/stores/branch-sync"
import { useRepository, useSelectedProjectPath, useSelectedRepositoryState } from "@/stores/repository"
import { closeRefusedUpdate, runUpdateProject, useRefusedUpdate } from "@/stores/update-project"

const UpdateBlockedDialog = lazy(() => import("@/components/repository/UpdateBlockedDialog").then(it => ({ default: it.UpdateBlockedDialog })))

/** The review status of a branch in the landing overview: its pull request and its Patronus robot (the shield only) */
function renderReviewChips(branchName: string) {
  return (
    <>
      <PullRequestChip branchName={branchName} />
      <PatronusChip branchName={branchName} compact />
    </>
  )
}

/** The toolbar of the main window, its title bar (`WindowTitleBar`) */
export function ConfigurationHeader() {
  const isSyncing = useBranchSync(state => state.isSyncing)
  const branchCount = useBranchSync(state => state.branchOrder.length)
  const hasFilterBar = useHasFilterBar(branchCount)
  const pathValidation = useRepository(state => state.pathValidation)
  const hasSelectedProject = useSelectedProjectPath() !== undefined
  const lastSyncTime = useSelectedRepositoryState("lastSyncTime")
  const refusedUpdate = useRefusedUpdate()
  // mounted on the first refusal and kept, so the dialog animates and returns the focus when it closes
  const [isRefusedDialogMounted, setRefusedDialogMounted] = useState(false)
  if (refusedUpdate !== null && !isRefusedDialogMounted) {
    setRefusedDialogMounted(true)
  }
  // the menu item Update Project (⌘T); for a local baseline the update says there is nothing to update from
  useAppCommand("update_project", () => {
    if (pathValidation.valid) {
      void runUpdateProject()
    }
  })

  return (
    <>
      <WindowTitleBar
        data-testid="configuration-header"
        end={
          // actions: incoming and the way into master together, the last sync time next to Sync, then Settings in every state
          <>
            {pathValidation.valid && (
              // hidden while empty: an empty flex item would still take a gap
              <div className="flex items-center gap-1 empty:hidden">
                <UpdateProjectButton />
                <LandingIndicator
                  renderPullRequest={renderReviewChips}
                  footer={
                    <>
                      <UpdateStateLine />
                      <PullRequestStatusLine />
                      <PatronusStatusLine />
                    </>
                  }
                />
                <ActivityButton />
              </div>
            )}
            {pathValidation.valid && lastSyncTime && (
              // stays as it is while a sync runs, so nothing moves: the spinner of Sync next to it says the sync
              <span className="hidden text-xs whitespace-nowrap text-muted-foreground @3xl:inline" data-testid="last-sync-time">
                <LastSyncTime lastSyncTime={lastSyncTime} />
              </span>
            )}
            {/* a failed sync shows as the red Sync with the error in its tooltip, next to the page alert */}
            <SyncButton />
            <SettingsButton />
          </>
        }
      >
        {/* the main menu replaces the native menu bar where the row is the title bar on Windows and Linux */}
        {hasCustomTitleBar && <MainMenuButton />}
        {/* repository info: gives way to the actions on a narrow window, the repository selector first */}
        <RepositorySelector disabled={isSyncing} />
        {/* where the user is: the view of the repository (Branches · Landing) */}
        {hasSelectedProject && pathValidation.valid && <MainViewSwitcher />}

        {/* the filter bar says it from FILTER_BAR_MIN_BRANCHES on ("All 35") */}
        {hasSelectedProject && branchCount > 0 && !hasFilterBar && pathValidation.valid && (
          <span className="hidden text-xs whitespace-nowrap text-muted-foreground @5xl:flex">
            {branchCount} {branchCount === 1 ? "branch" : "branches"}
          </span>
        )}
      </WindowTitleBar>
      {isRefusedDialogMounted && (
        <Suspense fallback={null}>
          <UpdateBlockedDialog check={refusedUpdate} onClose={closeRefusedUpdate} />
        </Suspense>
      )}
    </>
  )
}

/**
 * "Last synced 18 minutes ago", or "Checked just now" once a focus found nothing a sync reads changed since
 * (`syncIfChanged`): the branches are as current as a sync would make them, which the age of the sync alone hides
 */
function LastSyncTime({ lastSyncTime }: { lastSyncTime: number }) {
  const checkedAt = useBranchSync(state => state.inputsCheckedAt)
  if (checkedAt === null || checkedAt <= lastSyncTime) {
    return (
      <>
        {"Last synced "}
        <RelativeTime time={lastSyncTime} />
      </>
    )
  }
  return (
    <Tooltip>
      <TooltipTrigger render={<span data-testid="inputs-checked" />}>
        {"Checked "}
        <RelativeTime time={checkedAt} />
      </TooltipTrigger>
      <TooltipContent>
        {"Nothing a sync reads changed since the last sync, "}
        <RelativeTime time={lastSyncTime} />
      </TooltipContent>
    </Tooltip>
  )
}
