import { useTauriListen } from "@/hooks/useTauriListen"
import { REVEAL_BRANCH_EVENT, requestReveal, selectBranches } from "@/stores/commit-move"
import { isSelectedRepository } from "@/stores/repository"

/**
 * Show in Branch Deck of a notification of the IDE, and a branch a tab of Reviews or Landing shows
 * (openspec/specs/repository/ide-embedding/spec.md): `brd mcp` showed the main view and asks it to reveal the branch,
 * which it does as Go to of the palette does (expanded, scrolled into view, focused and selected) in the repository it
 * shows. Main window only.
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
