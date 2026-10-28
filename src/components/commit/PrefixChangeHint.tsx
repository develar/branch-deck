import { useBaselineOnRemote } from "@/hooks/useBaselineOnRemote"
import { usePreviewGroup } from "@/hooks/usePreviewGroup"
import { describePrefixChange } from "@/lib/stack"
import { getBranch, useStackParents, useStacksEnabled } from "@/stores/branch-sync"

export interface PrefixChangeHintProps {
  before: string
  after: string
  /** What applies the message: "Saving", "Squashing" */
  action: string
  /** The core of the action refuses a stack problem it would introduce (the reword, a regroup): the hint says so */
  refusesStackProblems?: boolean
}

/**
 * Warns that a changed subject moves the commit to another branch (or out of every branch); with stacked branches also
 * that it restacks the branch, or would make a parent conflict or name a parent without commits (built on the baseline).
 * A stack problem that the core of the action refuses (`refusesStackProblems`) is said to be refused. Rust groups both
 * messages (`preview_group`), the typed one once the typing pauses: until it answered, the hint of the last answer stays
 */
export function PrefixChangeHint({ before, after, action, refusesStackProblems = false }: PrefixChangeHintProps) {
  const stacks = useStacksEnabled()
  const stackParents = useStackParents()
  const baseline = useBaselineOnRemote()
  const from = usePreviewGroup(before).answer
  const to = usePreviewGroup(after).answer
  if (from === undefined || to === undefined) {
    return null
  }
  const change = describePrefixChange(
    from,
    to,
    action,
    stacks,
    baseline,
    name => {
      const branch = getBranch(name)
      return branch === undefined ? undefined : { declaredParent: branch.stack?.declaredParent ?? null, commitCount: branch.commitCount }
    },
    stackParents,
  )
  if (!change.warning) {
    return <span>{change.text}</span>
  }
  return (
    <span data-testid="commit-prefix-change" className="text-warning">
      {(refusesStackProblems ? change.refused : undefined) ?? change.text}
    </span>
  )
}
