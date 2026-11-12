import { useTauriListen } from "@/hooks/useTauriListen"
import { requestReveal, selectBranches } from "@/stores/commit-move"
import { isSelectedRepository } from "@/stores/repository"

/** The event of `brd mcp` that reveals a branch (`REVEAL_BRANCH_EVENT` of `crates/mcp-host/src/notices.rs`) */
const REVEAL_BRANCH_EVENT = "reveal-branch"

/**
 * Show in Branch Deck of a notification of the IDE (openspec/specs/repository/ide-embedding/spec.md): `brd mcp` showed
 * the main view and asks it to reveal the branch the notification is about, which it does as Go to of the palette does
 * (expanded, scrolled into view, focused and selected) in the repository it shows. Main window only.
 */
export function useRevealBranchEvent(): void {
  useTauriListen<{ repositoryPath: string; branchName: string }>(REVEAL_BRANCH_EVENT, ({ repositoryPath, branchName }) => {
    if (!isSelectedRepository(repositoryPath)) {
      return
    }
    requestReveal({ branchName, commitIds: [], focus: true })
    selectBranches(branchName, "only", [])
  })
}
