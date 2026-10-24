import { Activity as KeepMounted, lazy, Suspense, useEffect, useLayoutEffect, useRef, useState } from "react"
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
import { mainViewStore, showMainView, useMainView } from "@/stores/main-view"
import { getSelectedProject, initRepository, refreshBranchPrefix } from "@/stores/repository"
import { appSettingsStore } from "@/stores/settings/appSettings"
import { useSetupTracking } from "@/stores/setup"

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

// the statistics and their charts (Recharts): loaded the first time Landing shows, then kept mounted
const LandingView = lazy(() => import("@/components/landing/LandingView").then(it => ({ default: it.LandingView })))

// loaded on the first open, then kept mounted (a dialog animates and returns the focus when it closes)
const CreatePullRequestDialog = lazy(() => import("@/components/branch/CreatePullRequestDialog").then(it => ({ default: it.CreatePullRequestDialog })))

/** The width of the Activity side panel until its divider is dragged: the search and a line of an entry */
const ACTIVITY_PANEL_WIDTH = "24rem"

/**
 * The views keep what the user left (`spec/repository/main-views.spec.md`): both stay mounted (React's `Activity`, here
 * `KeepMounted`), so the rows, their forms and their selection live on; the scroll position and the focused element of
 * the branches are kept at the switch and given back on the return, as `display: none` drops the focus. The focus moves
 * into a view only when it was inside the one that hid, so a click on the switcher keeps it on the switcher.
 */
function useKeptViewPlace() {
  const view = useMainView()
  const branches = useRef<HTMLElement>(null)
  const landing = useRef<HTMLElement>(null)
  const landingHeading = useRef<HTMLHeadingElement>(null)
  const kept = useRef<{ element: HTMLElement | null; scrollTop: number; moveFocus: boolean }>({ element: null, scrollTop: 0, moveFocus: false })
  // at the switch, before React hides the view: what the user leaves
  useEffect(
    () =>
      mainViewStore.subscribe((state, previous) => {
        if (state.view === previous.view) {
          return
        }
        const active = document.activeElement instanceof HTMLElement && document.activeElement !== document.body ? document.activeElement : null
        const left = previous.view === "branches" ? branches.current : landing.current
        const moveFocus = active === null || (left?.contains(active) ?? false)
        if (previous.view === "branches") {
          kept.current = { element: moveFocus ? active : kept.current.element, scrollTop: branches.current?.scrollTop ?? 0, moveFocus }
        } else {
          kept.current = { ...kept.current, moveFocus }
        }
      }),
    [],
  )
  const isFirst = useRef(true)
  useLayoutEffect(() => {
    if (isFirst.current) {
      isFirst.current = false
      return
    }
    if (view === "branches") {
      if (branches.current !== null) {
        branches.current.scrollTop = kept.current.scrollTop
      }
      if (kept.current.moveFocus && kept.current.element?.isConnected === true) {
        kept.current.element.focus({ preventScroll: true })
      }
    } else if (kept.current.moveFocus) {
      landingHeading.current?.focus({ preventScroll: true })
    }
  }, [view])
  return { view, branches, landing, landingHeading }
}

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
    // a `git config` meanwhile: the names the page shows; the sync resolves the prefix in Rust itself
    refreshBranchPrefix()
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
  // the Setup dot: the main window also reads the CLIs now and then
  useSetupTracking({ readTools: true })
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
  // Branches · Landing (⌘1, ⌘2): the native View menu, ☰ and its keys, the palette
  const { view, branches, landing, landingHeading } = useKeptViewPlace()
  const [isLandingLoaded, setLandingLoaded] = useState(false)
  if (view === "landing" && !isLandingLoaded) {
    setLandingLoaded(true)
  }
  useAppCommand("show_branches", () => showMainView("branches"))
  useAppCommand("show_landing", () => showMainView("landing"))
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
            {/* each view in its own scroller, both kept mounted (`useKeptViewPlace`) */}
            <KeepMounted mode={view === "branches" ? "visible" : "hidden"}>
              <main ref={branches} className="h-full overflow-y-auto" data-testid="branches-view">
                <Container className="pb-2">
                  <BranchCreator />
                </Container>
                {/* after the content, out of its gaps: its room at the end moves no row */}
                <DragTargetBar />
              </main>
            </KeepMounted>
            <KeepMounted mode={view === "landing" ? "visible" : "hidden"}>
              <main ref={landing} className="h-full overflow-y-auto">
                <Container className="pb-2">
                  {isLandingLoaded && (
                    <Suspense fallback={null}>
                      <LandingView visible={view === "landing"} headingRef={landingHeading} />
                    </Suspense>
                  )}
                </Container>
              </main>
            </KeepMounted>
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
