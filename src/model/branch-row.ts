import { blockingBranchOf, stackProblemOf } from "@/lib/stack"
import type { Branch, BranchRowFacts } from "@/stores/branch-sync"

/**
 * What the row of `branch` shows beside its status (`Branch.row`), built anew: `branchOf` of src/model/store.ts keeps
 * the shown one while it is equal
 */
export function branchRowFacts(branch: Omit<Branch, "row">): BranchRowFacts {
  return {
    name: branch.name,
    commitCount: branch.commitCount,
    summary: branch.summary,
    descriptionTitle: branch.descriptionTitle,
    descriptionLead: branch.descriptionLead,
    nameIssueKey: branch.nameIssueKey,
    issueKey: branch.issueKey,
    hasError: branch.hasError,
    statusText: branch.statusText,
    remoteStatus: branch.remoteStatus,
    stack: branch.stack,
    blocked: branch.status === "Blocked",
    blockedBy: blockingBranchOf(branch),
    commitsHidden: branch.errorDetails?.Generic !== undefined && stackProblemOf(branch) === null,
  }
}
