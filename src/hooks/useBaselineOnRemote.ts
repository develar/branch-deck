import { baselineOnRemote } from "@/lib/stack"
import { useBranchSyncField } from "@/stores/branch-sync"

/** "master" for `origin/master`: where a branch is built when no branch with commits is below it */
export function useBaselineOnRemote(): string {
  const baselineBranch = useBranchSyncField("baselineBranch", baseline => baseline)
  return baselineBranch === null ? "the baseline" : baselineOnRemote(baselineBranch)
}
