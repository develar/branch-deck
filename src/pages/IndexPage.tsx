import { Container } from "@/components/layout/Container"

export function IndexPage() {
  // TODO(Phase 9): useAutoUpdate().checkForUpdatesOnStartup() on mount
  return (
    <Container className="pb-4" data-testid="app-container">
      {/* TODO(Phase 4): <BranchCreator /> */}
      <p className="py-6 text-sm text-muted-foreground">BranchCreator (TODO Phase 4)</p>
    </Container>
  )
}
