import { lazy, Suspense, useState } from "react"
import pDebounce from "p-debounce"
import { BranchCreator } from "@/components/branch/BranchCreator"
import { Container } from "@/components/layout/Container"
import { ConfigurationHeader } from "@/components/repository/ConfigurationHeader"
import { DockStatus } from "@/components/repository/DockStatus"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useAutoUpdate } from "@/hooks/useAutoUpdate"
import { useBranchReviewLink } from "@/hooks/useBranchReviewLink"
import { useInstallAgentCli } from "@/hooks/useInstallAgentCli"
import { useMenuSync } from "@/hooks/useMenuSync"
import { useRepositoryConfigChanges } from "@/hooks/useRepositoryConfigChanges"
import { useReviewWindowBridge } from "@/hooks/useReviewWindowBridge"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { notifyError } from "@/lib/errors"
import { SETTINGS_WINDOW } from "@/lib/settings-window"
import type { SettingsTarget } from "@/lib/settings-window"
import { openSubWindow } from "@/lib/window-management"
import { syncIfChanged } from "@/stores/branch-sync"
import { useCreatePullRequestBranch } from "@/stores/create-pull-request-dialog"
import { getSelectedProject } from "@/stores/repository"
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

/**
 * The main window: the toolbar on the canvas, then the islands in their own scroller, so the toolbar stays where the
 * title bar is and a row scrolled into view never lands under it.
 */
export function IndexPage() {
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
      <main className="min-h-0 flex-1 overflow-y-auto">
        <Container className="pb-2">
          <BranchCreator />
        </Container>
      </main>
      <Suspense fallback={null}>{isCreatePullRequestLoaded && <CreatePullRequestDialog />}</Suspense>
    </div>
  )
}
