import { describePrefixChange } from "@/lib/stack"
import { getBranch, useBranchSync, useIssueGroups, useStacksEnabled } from "@/stores/branch-sync"

export interface PrefixChangeHintProps {
  before: string
  after: string
  /** What applies the message: "Saving", "Squashing" */
  action: string
}

/**
 * Warns that a changed subject moves the commit to another branch (or out of every branch); with stacked branches also
 * that it restacks the branch, or would make a parent conflict or name a parent without commits (built on the baseline).
 */
export function PrefixChangeHint({ before, after, action }: PrefixChangeHintProps) {
  const stacks = useStacksEnabled()
  const issueGroups = useIssueGroups()
  const baselineBranch = useBranchSync(state => state.baselineBranch) ?? "the baseline"
  const change = describePrefixChange(before, after, action, stacks, baselineBranch, (name) => {
    const branch = getBranch(name)
    return branch === undefined ? undefined : { declaredParent: branch.stack?.declaredParent ?? null, commitCount: branch.commitCount }
  }, issueGroups)
  if (!change.warning) {
    return <span>{change.text}</span>
  }
  return <span data-testid="commit-prefix-change" className="text-warning">{change.text}</span>
}
