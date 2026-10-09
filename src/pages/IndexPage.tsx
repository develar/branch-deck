import { BranchCreator } from "@/components/branch/BranchCreator"
import { Container } from "@/components/layout/Container"

export function IndexPage() {
  // TODO(Phase 9): useAutoUpdate().checkForUpdatesOnStartup() on mount
  return (
    <Container className="pb-4" data-testid="app-container">
      <BranchCreator />
    </Container>
  )
}
