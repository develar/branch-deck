import { BranchCreator } from "@/components/branch/BranchCreator"
import { Container } from "@/components/layout/Container"
import { ConfigurationHeader } from "@/components/repository/ConfigurationHeader"
import { useAutoUpdate } from "@/hooks/useAutoUpdate"
import { useInstallAgentCli } from "@/hooks/useInstallAgentCli"

/**
 * The main window: the toolbar on the canvas, then the islands in their own scroller, so the toolbar stays where the
 * title bar is and a row scrolled into view never lands under it.
 */
export function IndexPage() {
  // startup update check and the "Check for Updates…" menu item
  useAutoUpdate()
  // the "Install Agent CLI…" menu item
  useInstallAgentCli()
  return (
    <div className="flex h-dvh flex-col" data-testid="app-container">
      <ConfigurationHeader />
      <main className="min-h-0 flex-1 overflow-y-auto">
        <Container className="pb-2">
          <BranchCreator />
        </Container>
      </main>
    </div>
  )
}
