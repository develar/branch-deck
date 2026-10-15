import type { PatronusPushMode } from "@/lib/bindings"
import { joinNames } from "@/lib/text"

/** What a Safe Push or a Dry Run start means for a stacked branch (`describeStackStart`) */
export interface StackStartNotes {
  /** Root first: the Safe Push of the root-most ancestor the form offers before its own start */
  lead: { text: string, root: string } | null
  /** What the start does to the other branches of the stack, muted */
  note: string | null
}

export interface StackStart {
  branchName: string
  mode: PatronusPushMode
  /** The target branch of Patronus ("master") */
  target: string
  /** The branches the branch is built on, root first (`stackAncestors`) */
  ancestors: readonly string[]
  /** The branches built on the branch */
  children: readonly string[]
  /** The open pull request of the branch the start doesn't comment on (its base is the parent's branch); null without one */
  pullRequest: number | null
  /** A child has an open pull request: its push retargets it */
  childrenHavePullRequests: boolean
}

/**
 * A stacked branch carries the commits of the branches it is built on. A Safe Push of it leads root first and says that
 * its own start lands the ancestors too (and that the approvals of a pull request into the parent's branch don't
 * count); a dry run checks their commits too; a Safe Push of a parent says that its children move onto the target after
 * the merge.
 */
export function describeStackStart({ branchName, mode, target, ancestors, children, pullRequest, childrenHavePullRequests }: StackStart): StackStartNotes {
  const [root] = ancestors
  if (mode === "dryRun") {
    return { lead: null, note: root === undefined ? null : `It checks the commits of ${joinNames(ancestors)} too: ${branchName} is built on them.` }
  }
  if (root !== undefined) {
    const isAre = ancestors.length === 1 ? "isn't" : "aren't"
    const next = ancestors.length === 1 ? `then ${branchName}` : "then the next"
    const lead = `${branchName} is built on ${joinNames(ancestors)}, which ${isAre} in ${target} yet. Safe Push ${root} first, ${next}: after a merge, the next sync moves the branches built on it onto ${target}.`
    const approvals = pullRequest === null ? "" : ` The approvals of pull request #${pullRequest} don't count: it compares ${branchName} with ${ancestors.at(-1)}, and Patronus merges a pull request into its base.`
    return { lead: { text: lead, root }, note: `Or start ${branchName} now: the Safe Push lands ${joinNames(ancestors)} with it.${approvals}` }
  }
  if (children.length > 0) {
    const verb = children.length === 1 ? "is" : "are"
    const pullRequests = childrenHavePullRequests ? `; their pull requests are retargeted to ${target} when they are pushed` : ""
    return { lead: null, note: `${joinNames(children)} ${verb} built on ${branchName}: after the merge, the next sync moves them onto ${target}${pullRequests}.` }
  }
  return { lead: null, note: null }
}
