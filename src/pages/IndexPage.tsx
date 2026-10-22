import { lazy, Suspense, useEffect, useRef, useState } from "react"
import pDebounce from "p-debounce"
import { useDefaultLayout } from "react-resizable-panels"
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { BranchCreator } from "@/components/branch/BranchCreator"
import { DragTargetBar } from "@/components/branch/DragTargetBar"
import { CommandPalette } from "@/components/command-palette/CommandPalette"
import { Container } from "@/components/layout/Container"
import { ActivityPanel } from "@/components/repository/ActivityPanel"
import { ConfigurationHeader } from "@/components/repository/ConfigurationHeader"
import { DockStatus } from "@/components/repository/DockStatus"
import { ACTIVITY_PANEL_MIN_WIDTH, useActivityPanel } from "@/hooks/useActivityPanel"
import { useActivityWindowLink } from "@/hooks/useActivityWindowLink"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useAutoUpdate } from "@/hooks/useAutoUpdate"
import { useBranchReviewLink } from "@/hooks/useBranchReviewLink"
import { useInstallAgentCli } from "@/hooks/useInstallAgentCli"
import { useMenuSync } from "@/hooks/useMenuSync"
import { useRepositoryConfigChanges } from "@/hooks/useRepositoryConfigChanges"
import { useReviewStatusRefresh } from "@/hooks/useReviewStatusRefresh"
import { useReviewWindowBridge } from "@/hooks/useReviewWindowBridge"
import { useSyncOnOpen } from "@/hooks/useSyncOnOpen"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { notifyError } from "@/lib/errors"
import { SETTINGS_WINDOW } from "@/lib/settings-window"
import type { SettingsTarget } from "@/lib/settings-window"
import { openSubWindow } from "@/lib/window-management"
import { MIN_ISLANDS_WIDTH } from "@/lib/zoom"
import { syncIfChanged } from "@/stores/branch-sync"
import { useCreatePullRequestBranch } from "@/stores/create-pull-request-dialog"
import { getSelectedProject, initRepository } from "@/stores/repository"
import { appSettingsStore } from "@/stores/settings/appSettings"

/**
 * Window focus handler: with "Auto-sync on Focus" (on by default) and a project selected, syncs when what a sync reads
 * changed since the last one (`syncIfChanged`; during a sync, it checks once after it)
 */
const debouncedAutoSync = pDebounce(async () => {
  if (appSettingsStore.getState().autoSyncOnFocus && getSelectedProject() !== null) {
    await syncIfChanged()
  }
}, 100)

/** Opens the Settings window, or brings it to the front at `target` */
function openSettingsWindow(target: SettingsTarget): void {
  openSubWindow({ windowId: SETTINGS_WINDOW, url: "/settings", title: "Settings", width: 720, height: 540, data: target }).catch((error: unknown) =>
    notifyError("Cannot open Settings", error),
  )
}

// loaded on the first open, then kept mounted (a dialog animates and returns the focus when it closes)
const CreatePullRequestDialog = lazy(() => import("@/components/branch/CreatePullRequestDialog").then(it => ({ default: it.CreatePullRequestDialog })))

/** The width of the Activity side panel until its divider is dragged: the search and a line of an entry */
const ACTIVITY_PANEL_WIDTH = "24rem"

/**
 * The main window: the toolbar on the canvas, then the islands in their own scroller, so the toolbar stays where the
 * title bar is and a row scrolled into view never lands under it; beside them the Activity side panel, an island of its
 * own with its divider, where the window has room for it.
 */
export function IndexPage() {
  // the repository of the window, its sync when the window opens it, and its review status: whichever view shows
  useEffect(() => {
    initRepository()
  }, [])
  useReviewStatusRefresh()
  useSyncOnOpen()
  // "Auto-sync on Focus": the item of the native menu, and the sync when the window gets the focus
  useMenuSync()
  useWindowFocus(() => {
    void debouncedAutoSync()
  })
  // "Settings…" (⌘,) of every window and the buttons that open a page of it: only this window opens it, with the
  // current settings as its snapshot
  useAppCommand<SettingsTarget | null>("open_settings", target => openSettingsWindow(target ?? {}))
  // a write of Settings → Repository: the sync, the branch prefix, the pull requests it changes
  useRepositoryConfigChanges()
  // startup update check and the "Check for Updates…" menu item
  useAutoUpdate()
  // the "Install Agent CLI…" menu item
  useInstallAgentCli()
  // the review window of a branch: the tips a sync built, what it can do with its branches, and what it asks for
  useBranchReviewLink()
  // the review window of a pull request: the state of its requests, and the writes it asks for
  useReviewWindowBridge()
  // "Activity" of every window: the log of the selected repository in the side panel, or in its window, and what the
  // window asks for
  const area = useRef<HTMLDivElement>(null)
  const isActivityPanelShown = useActivityPanel(area)
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({ id: "activity-panel", storage: localStorage, onlySaveAfterUserInteractions: true })
  useActivityWindowLink()
  // "Create Pull Request…" of a branch (the review window has its own, `WindowCreatePullRequestDialog`)
  const createPullRequestOpen = useCreatePullRequestBranch() !== null
  const [isCreatePullRequestLoaded, setCreatePullRequestLoaded] = useState(false)
  if (createPullRequestOpen && !isCreatePullRequestLoaded) {
    setCreatePullRequestLoaded(true)
  }
  return (
    <div className="flex h-dvh flex-col" data-testid="app-container">
      <ConfigurationHeader />
      {/* the running robots and what needs the user, in the Dock */}
      <DockStatus />
      <div ref={area} className="min-h-0 flex-1">
        <ResizablePanelGroup orientation="horizontal" defaultLayout={isActivityPanelShown ? defaultLayout : undefined} onLayoutChanged={onLayoutChanged}>
          <ResizablePanel id="main" minSize={`${MIN_ISLANDS_WIDTH}px`}>
            <main className="h-full overflow-y-auto">
              <Container className="pb-2">
                <BranchCreator />
              </Container>
              {/* after the content, out of its gaps: its room at the end moves no row */}
              <DragTargetBar />
            </main>
          </ResizablePanel>
          {isActivityPanelShown && (
            <>
              {/* the gap between the islands is the divider */}
              <ResizableHandle className="w-2 bg-transparent" />
              <ResizablePanel id="activity" defaultSize={ACTIVITY_PANEL_WIDTH} minSize={`${ACTIVITY_PANEL_MIN_WIDTH}px`} maxSize="50">
                <div className="h-full pr-2 pb-2">
                  <ActivityPanel />
                </div>
              </ResizablePanel>
            </>
          )}
        </ResizablePanelGroup>
      </div>
      <CommandPalette />
      <Suspense fallback={null}>{isCreatePullRequestLoaded && <CreatePullRequestDialog />}</Suspense>
    </div>
  )
}
