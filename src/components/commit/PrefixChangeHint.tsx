import { groupFor } from "@/lib/commit"

export interface PrefixChangeHintProps {
  before: string
  after: string
  /** What applies the message: "Saving", "Squashing" */
  action: string
}

/** Warns that a changed subject moves the commit to another branch (or out of every branch). */
export function PrefixChangeHint({ before, after, action }: PrefixChangeHintProps) {
  const branch = groupFor(after)?.branch ?? null
  if (branch === (groupFor(before)?.branch ?? null)) {
    return <span>The first line is the subject</span>
  }
  return (
    <span data-testid="commit-prefix-change" className="text-warning">
      {branch === null ? `${action} moves the commit to the unassigned commits` : `${action} moves the commit to the branch ${branch}`}
    </span>
  )
}
