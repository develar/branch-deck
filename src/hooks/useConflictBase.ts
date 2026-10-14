import { useBranchSync } from "@/stores/branch-sync"

/**
 * The branch the commits of `branchName` are copied onto: the parent of a stacked branch, else the baseline; null before
 * the first sync names it. The conflict titles say a commit "does not apply cleanly onto" it.
 */
export function useConflictBase(branchName: string | undefined): string | null {
  return useBranchSync(state => (branchName === undefined ? null : state.branches[branchName]?.stack?.effectiveBase) ?? state.baselineBranch)
}
