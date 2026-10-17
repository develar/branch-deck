import { lazy, Suspense, useState } from "react"
import { Settings2Icon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Switch } from "@/components/ui/switch"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { PatronusChip } from "@/components/branch/PatronusChip"
import { PullRequestChip } from "@/components/branch/PullRequestChip"
import { ActivityButton } from "@/components/repository/ActivityButton"
import { MainMenuButton } from "@/components/repository/MainMenuButton"
import { OutgoingIndicator } from "@/components/repository/OutgoingIndicator"
import { PatronusStatusLine } from "@/components/repository/PatronusStatusLine"
import { PullRequestStatusLine } from "@/components/repository/PullRequestStatusLine"
import { RepositorySelector } from "@/components/repository/RepositorySelector"
import { RobotsIndicator } from "@/components/repository/RobotsIndicator"
import { SyncButton } from "@/components/repository/SyncButton"
import { UpdateProjectButton, UpdateStateLine } from "@/components/repository/UpdateProjectButton"
import { RelativeTime } from "@/components/shared/RelativeTime"
import { WindowTitleBar } from "@/components/shared/WindowTitleBar"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useHasFilterBar } from "@/hooks/useBranchFilter"
import { useOutgoingAttention } from "@/hooks/useOutgoingAttention"
import { usePullRequestOverview } from "@/hooks/usePullRequestOverview"
import { hasCustomTitleBar } from "@/lib/title-bar"
import { useBranchSync } from "@/stores/branch-sync"
import { useRepository, useSelectedProjectField, useSelectedProjectPath } from "@/stores/repository"
import { setAutoSyncOnFocus, useAppSettings } from "@/stores/settings/appSettings"
import { closeRefusedUpdate, runUpdateProject, useRefusedUpdate } from "@/stores/update-project"

const UpdateBlockedDialog = lazy(() => import("@/components/repository/UpdateBlockedDialog").then(it => ({ default: it.UpdateBlockedDialog })))

/** The review status of a branch in the outgoing overview: its pull request and its Patronus robot (the shield only) */
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
  const autoSyncOnFocus = useAppSettings(state => state.autoSyncOnFocus)
  const lastSyncTime = useSelectedProjectField("lastSyncTime")
  const refusedUpdate = useRefusedUpdate()
  const pullRequestOverview = usePullRequestOverview()
  const attention = useOutgoingAttention()
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
          // actions: incoming and outgoing together, the last sync time next to Sync and its settings
          <>
            {pathValidation.valid && (
              // hidden while empty: an empty flex item would still take a gap
              <div className="flex items-center gap-1 empty:hidden">
                <UpdateProjectButton />
                <OutgoingIndicator
                  renderPullRequest={renderReviewChips}
                  reviewBranches={pullRequestOverview.openBranches}
                  attention={attention}
                  footer={
                    <>
                      <UpdateStateLine />
                      <PullRequestStatusLine />
                      <PatronusStatusLine />
                    </>
                  }
                />
                <RobotsIndicator />
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
            <div className="flex items-center gap-1">
              <SyncButton />
              <Popover>
                <Tooltip>
                  <PopoverTrigger render={<TooltipTrigger render={<Button variant="ghost" size="icon-sm" aria-label="Sync settings" data-testid="sync-settings-button" />} />}>
                    <Settings2Icon />
                  </PopoverTrigger>
                  <TooltipContent>Sync settings</TooltipContent>
                </Tooltip>
                <PopoverContent align="end" className="w-auto min-w-[200px] gap-2 p-3">
                  <label className="flex items-center gap-2 text-sm">
                    <Switch size="sm" checked={autoSyncOnFocus} onCheckedChange={setAutoSyncOnFocus} />
                    Auto-sync on Focus
                  </label>
                  <div className="text-xs text-muted-foreground">Sync when the window gets the focus and a commit, a fetch or a push changed the branches</div>
                </PopoverContent>
              </Popover>
            </div>
          </>
        }
      >
        {/* the main menu replaces the native menu bar where the row is the title bar on Windows and Linux */}
        {hasCustomTitleBar && <MainMenuButton />}
        {/* repository info: gives way to the actions on a narrow window, the repository selector first */}
        <RepositorySelector disabled={isSyncing} />

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
