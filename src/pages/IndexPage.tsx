import pDebounce from "p-debounce"
import { BranchCreator } from "@/components/branch/BranchCreator"
import { Container } from "@/components/layout/Container"
import { ConfigurationHeader } from "@/components/repository/ConfigurationHeader"
import { DockStatus } from "@/components/repository/DockStatus"
import { useAutoUpdate } from "@/hooks/useAutoUpdate"
import { useBranchReviewLink } from "@/hooks/useBranchReviewLink"
import { useInstallAgentCli } from "@/hooks/useInstallAgentCli"
import { useMenuSync } from "@/hooks/useMenuSync"
import { useReviewWindowBridge } from "@/hooks/useReviewWindowBridge"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { syncIfChanged } from "@/stores/branch-sync"
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
  // startup update check and the "Check for Updates…" menu item
  useAutoUpdate()
  // the "Install Agent CLI…" menu item
  useInstallAgentCli()
  // the review window of a branch: the tips a sync built, what it can do with its branches, and what it asks for
  useBranchReviewLink()
  // the review window of a pull request: the state of its requests, and the writes it asks for
  useReviewWindowBridge()
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
    </div>
  )
}
