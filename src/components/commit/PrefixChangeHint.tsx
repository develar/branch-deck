import { describePrefixChange } from "@/lib/stack"
import { getBranch, useStacksEnabled } from "@/stores/branch-sync"

export interface PrefixChangeHintProps {
  before: string
  after: string
  /** What applies the message: "Saving", "Squashing" */
  action: string
}

/**
 * Warns that a changed subject moves the commit to another branch (or out of every branch); with stacked branches also
 * that it restacks the branch, or would make a parent conflict or a missing parent.
 */
export function PrefixChangeHint({ before, after, action }: PrefixChangeHintProps) {
  const stacks = useStacksEnabled()
  const change = describePrefixChange(before, after, action, stacks, (name) => {
    const branch = getBranch(name)
    return branch === undefined ? undefined : { declaredParent: branch.stack?.declaredParent ?? null, commitCount: branch.commitCount }
  })
  if (!change.warning) {
    return <span>{change.text}</span>
  }
  return <span data-testid="commit-prefix-change" className="text-warning">{change.text}</span>
}
