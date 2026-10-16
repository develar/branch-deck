import { useMemo } from "react"
import type { MissingCommit } from "@/lib/bindings"
import { getMissingCommitOwners } from "@/lib/merge-conflict"
import type { MissingCommitOwners } from "@/lib/merge-conflict"
import { useBranchSync } from "@/stores/branch-sync"

/** The missing commits of a conflict by the branch they are in, with the stack that holds them all */
export function useMissingCommitOwners(missingCommits: readonly MissingCommit[]): MissingCommitOwners {
  const branches = useBranchSync(state => state.branches)
  return useMemo(() => getMissingCommitOwners(missingCommits, name => branches[name]?.stack?.effectiveBase), [missingCommits, branches])
}
