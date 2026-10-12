import { BranchCreator } from "@/components/branch/BranchCreator"
import { Container } from "@/components/layout/Container"
import { useAutoUpdate } from "@/hooks/useAutoUpdate"

export function IndexPage() {
  // startup update check and the "Check for Updates…" menu item
  useAutoUpdate()
  return (
    <Container className="py-4" data-testid="app-container">
      <BranchCreator />
    </Container>
  )
}
