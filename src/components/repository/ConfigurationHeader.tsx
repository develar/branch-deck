import { lazy, Suspense, useState } from "react"
import { Settings2Icon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Switch } from "@/components/ui/switch"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ActivityButton } from "@/components/repository/ActivityButton"
import { PatronusChip } from "@/components/branch/PatronusChip"
import { PullRequestChip } from "@/components/branch/PullRequestChip"
import { OutgoingIndicator } from "@/components/repository/OutgoingIndicator"
import { PatronusStatusLine } from "@/components/repository/PatronusStatusLine"
import { PullRequestStatusLine } from "@/components/repository/PullRequestStatusLine"
import { RepositorySelector } from "@/components/repository/RepositorySelector"
import { RobotsIndicator } from "@/components/repository/RobotsIndicator"
import { SyncButton } from "@/components/repository/SyncButton"
import { UpdateProjectButton, UpdateStateLine } from "@/components/repository/UpdateProjectButton"
import { WindowControls } from "@/components/repository/WindowControls"
import { RelativeTime } from "@/components/shared/RelativeTime"
import { useHasFilterBar } from "@/hooks/useBranchFilter"
import { useOutgoingAttention } from "@/hooks/useOutgoingAttention"
import { usePullRequestOverview } from "@/hooks/usePullRequestOverview"
import { useTauriListen } from "@/hooks/useTauriListen"
import { useWindowFrameState } from "@/hooks/useWindowFrameState"
import { titleBar } from "@/lib/title-bar"
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

/**
 * The toolbar of the main window, on the canvas above the islands. Where the page draws the title bar (`titleBar`) it is
 * the title bar: its empty parts drag the window, on macOS it starts after the traffic lights (not in fullscreen, where
 * they are hidden), on Windows it ends with the window buttons.
 */
export function ConfigurationHeader() {
  const frame = useWindowFrameState()
  const isSyncing = useBranchSync(state => state.isSyncing)
  const branchCount = useBranchSync(state => state.branchOrder.length)
  const hasFilterBar = useHasFilterBar(branchCount)
  const pathValidation = useRepository(state => state.pathValidation)
  const hasSelectedProject = useSelectedProjectPath() !== undefined
  const autoSyncOnFocus = useAppSettings(state => state.autoSyncOnFocus)
  const zoom = useAppSettings(state => state.zoom)
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
  useTauriListen("update-project", () => {
    if (pathValidation.valid) {
      void runUpdateProject()
    }
  })

  return (
    <header
      className={cn(
        // a container: its width at its own size, not the viewport of the zoomed page, hides the secondary texts
        "@container flex h-10 shrink-0 items-center gap-3 select-none",
        titleBar === "overlay" && !frame.fullscreen ? "pl-22" : "pl-2",
        titleBar === "custom" ? "pr-0" : "pr-2",
      )}
      data-testid="configuration-header"
      data-title-bar={titleBar ?? "native"}
      data-tauri-drag-region={titleBar === null ? undefined : "deep"}
      // the title bar keeps the size of the window chrome, the page zoom (`useZoom`) leaves it alone
      style={zoom === 1 ? undefined : { zoom: 1 / zoom }}
    >
      <div className="flex min-w-0 flex-1 items-center justify-between gap-4">
        {/* repository info: gives way to the actions on a narrow window, the repository selector first */}
        <div className="flex min-w-0 flex-1 items-center gap-3">
          <RepositorySelector disabled={isSyncing} />

          {/* the filter bar says it from FILTER_BAR_MIN_BRANCHES on ("All 35") */}
          {hasSelectedProject && branchCount > 0 && !hasFilterBar && pathValidation.valid && (
            <span className="hidden text-xs whitespace-nowrap text-muted-foreground @5xl:flex">
              {branchCount}
              {" "}
              {branchCount === 1 ? "branch" : "branches"}
            </span>
          )}
        </div>

        {/* actions: incoming and outgoing together, the last sync time next to Sync and its settings */}
        <div className="flex shrink-0 items-center gap-3">
          {pathValidation.valid && (
            // hidden while empty: an empty flex item would still take a gap
            <div className="flex items-center gap-1 empty:hidden">
              <UpdateProjectButton />
              <OutgoingIndicator
                renderPullRequest={renderReviewChips}
                reviewBranches={pullRequestOverview.openBranches}
                attention={attention}
                footer={(
                  <>
                    <UpdateStateLine />
                    <PullRequestStatusLine />
                    <PatronusStatusLine />
                  </>
                )}
              />
              <RobotsIndicator />
              <ActivityButton />
            </div>
          )}
          {pathValidation.valid && (lastSyncTime || isSyncing) && (
            // both texts share one cell, so the controls before it stay in place while a sync runs
            <div className="hidden text-xs whitespace-nowrap text-muted-foreground @3xl:grid" data-testid="last-sync-time">
              <span className={cn("col-start-1 row-start-1 text-right", isSyncing && "invisible")}>
                {lastSyncTime ? <LastSyncTime lastSyncTime={lastSyncTime} /> : null}
              </span>
              <span className={cn("col-start-1 row-start-1 text-right", !isSyncing && "invisible")}>Syncing…</span>
            </div>
          )}
          {/* a failed sync shows as the red Sync with the error in its tooltip, next to the page alert */}
          <div className="flex items-center gap-1">
            <SyncButton />
            <Popover>
              <Tooltip>
                <PopoverTrigger
                  render={<TooltipTrigger render={<Button variant="ghost" size="icon-sm" aria-label="Sync settings" data-testid="sync-settings-button" />} />}
                >
                  <Settings2Icon />
                </PopoverTrigger>
                <TooltipContent>Sync settings</TooltipContent>
              </Tooltip>
              <PopoverContent align="end" className="w-auto min-w-[200px] gap-2 p-3">
                <label className="flex items-center gap-2 text-sm">
                  <Switch size="sm" checked={autoSyncOnFocus} onCheckedChange={setAutoSyncOnFocus} />
                  Auto-sync on Focus
                </label>
                <div className="text-xs text-muted-foreground">
                  Sync when the window gets the focus and a commit, a fetch or a push changed the branches
                </div>
              </PopoverContent>
            </Popover>
          </div>
        </div>
      </div>
      {titleBar === "custom" && <WindowControls maximized={frame.maximized} />}
      {isRefusedDialogMounted && (
        <Suspense fallback={null}>
          <UpdateBlockedDialog check={refusedUpdate} onClose={closeRefusedUpdate} />
        </Suspense>
      )}
    </header>
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
