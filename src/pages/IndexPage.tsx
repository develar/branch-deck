import { BranchCreator } from "@/components/branch/BranchCreator"
import { Container } from "@/components/layout/Container"
import { useAutoUpdate } from "@/hooks/useAutoUpdate"
import { useInstallAgentCli } from "@/hooks/useInstallAgentCli"

export function IndexPage() {
  // startup update check and the "Check for Updates…" menu item
  useAutoUpdate()
  // the "Install Agent CLI…" menu item
  useInstallAgentCli()
  return (
    <Container className="py-4" data-testid="app-container">
      <BranchCreator />
    </Container>
  )
}
