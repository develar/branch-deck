import type { PatronusPushMode, StackCommit, StackIndependence, StackUse } from "@/lib/bindings"
import { formatBranchesAndCommits } from "@/lib/branch"
import { formatCommitCount } from "@/lib/commit"
import { joinNames } from "@/lib/text"

/** From this many ancestors on, the notes count them and their commits: the names alone hide the size of the stack */
const COUNTED_ANCESTORS = 3

/**
 * The first action of the lead: the Safe Push of the root-most ancestor, or, when the branch builds alone, the unstack
 * from its parent, or, when it needs commits it can take, the move of them into the branch. A branch that applies alone
 * but uses a name of its stack leads root first and still offers the unstack after it (`unstack`): names, not a
 * compiler, found the use
 */
export type StackLeadAction = { kind: "safePushRoot"; unstack?: string } | { kind: "unstack"; parent: string } | { kind: "move"; commits: readonly StackCommit[]; label: string }

export interface StackLead {
  text: string
  /** The root-most ancestor: its Safe Push is the first action, else a link after it */
  root: string
  action: StackLeadAction
}

/** What a Safe Push or a Dry Run start means for a stacked branch (`describeStackStart`) */
export interface StackStartNotes {
  /** Root first, unless the branch builds alone or can take the commits it needs: the lead offers that first */
  lead: StackLead | null
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
  /** The commits of `ancestors` */
  ancestorCommitCount: number
  /** The branches built on the branch */
  children: readonly string[]
  /** The open pull request of the branch the start doesn't comment on (its base is the parent's branch); null without one */
  pullRequest: number | null
  /** A child has an open pull request: its push retargets it */
  childrenHavePullRequests: boolean
  /** The trial build of the branch alone (`get_stack_independence`); null before it answers, when it failed, or for a root */
  independence: StackIndependence | null
}

/** "Cache the icon" changes lines that "Parse the icon" of IJPL-1 adds; with more commits, "2 commits of IJPL-1 add" */
function describeNeed(child: StackCommit, needed: readonly StackCommit[]): string {
  const [first] = needed
  if (needed.length === 1 && first !== undefined) {
    return `"${child.subject}" changes lines that "${first.subject}" of ${first.branch} adds.`
  }
  const branches = [...new Set(needed.map(commit => commit.branch))]
  return `"${child.subject}" changes lines that ${formatCommitCount(needed.length)} of ${joinNames(branches)} add.`
}

/** "Start the lane" uses LaneDocument, which "Add the document" of air-lane adds; with more names, "and 2 more names" */
function describeUses(uses: readonly StackUse[]): string {
  const [first] = uses
  if (first === undefined) {
    return ""
  }
  const more = uses.length === 1 ? "" : uses.length === 2 ? " and 1 more name" : ` and ${uses.length - 1} more names`
  return `"${first.commit.subject}" uses ${first.name}${more}, which "${first.declaredBy.subject}" of ${first.declaredBy.branch} adds.`
}

/**
 * The lead of a stacked branch's Safe Push. It builds alone: the unstack first, unless it uses a name of its stack
 * (`branchdeck.symbols on`), which leads root first with the unstack after it. It needs commits that can move: the move
 * first. Otherwise root first, and the commits it needs, or its conflict, are named. The checks are lines and names,
 * not a compiler, so the unstack is offered, never done.
 */
function stackLead(branchName: string, target: string, ancestors: readonly string[], root: string, independence: StackIndependence | null): StackLead {
  const isAre = ancestors.length === 1 ? "isn't" : "aren't"
  const next = ancestors.length === 1 ? `then ${branchName}` : "then the next"
  const rootFirst = `${branchName} is built on ${joinNames(ancestors)}, which ${isAre} in ${target} yet. Safe Push ${root} first, ${next}: after a merge, the next sync moves the branches built on it onto ${target}.`
  if (independence === null) {
    return { text: rootFirst, root, action: { kind: "safePushRoot" } }
  }
  if (independence.kind === "builds") {
    const { parent, uses, symbols } = independence
    if (uses.length > 0) {
      const text = `${describeUses(uses)} ${branchName} applies on the baseline without ${parent}, but needs it to compile. Found by names, not by a compiler.`
      return { text, root, action: { kind: "safePushRoot", unstack: parent } }
    }
    // the trial build is on the base the sync builds on, not on the newest target
    const checked = symbols ? ` and uses no name it adds. Not checked by a compiler.` : `. Compile dependencies are not checked.`
    return { text: `${branchName} applies on the baseline without ${parent}${checked}`, root, action: { kind: "unstack", parent } }
  }
  const { childCommit, parentCommits, movable, files } = independence
  if (movable) {
    const count = parentCommits.length
    const label = count === 1 ? "Move 1 Commit Here" : `Move ${count} Commits Here`
    const then = `${branchName} may build without ${independence.parent} once ${count === 1 ? "it is" : "they are"} here.`
    return { text: `${describeNeed(childCommit, parentCommits)} ${then}`, root, action: { kind: "move", commits: parentCommits, label } }
  }
  const need = parentCommits.length === 0 ? `Without ${independence.parent}, "${childCommit.subject}" conflicts in ${joinNames(files)}.` : describeNeed(childCommit, parentCommits)
  return { text: `${rootFirst} ${need}`, root, action: { kind: "safePushRoot" } }
}

/**
 * A stacked branch carries the commits of the branches it is built on. A Safe Push of it leads root first and says that
 * its own start lands the ancestors too (and that the approvals of a pull request into the parent's branch don't
 * count); a dry run checks their commits too; a Safe Push of a parent says that its children move onto the target after
 * the merge. A deep stack is counted first: "the Safe Push lands 21 branches · 140 commits with it: a, b, c and 18 more".
 */
export function describeStackStart({
  branchName,
  mode,
  target,
  ancestors,
  ancestorCommitCount,
  children,
  pullRequest,
  childrenHavePullRequests,
  independence,
}: StackStart): StackStartNotes {
  const [root] = ancestors
  const counted = ancestors.length >= COUNTED_ANCESTORS ? formatBranchesAndCommits(ancestors.length, ancestorCommitCount) : null
  if (mode === "dryRun") {
    if (root === undefined) {
      return { lead: null, note: null }
    }
    const note =
      counted === null
        ? `It checks the commits of ${joinNames(ancestors)} too: ${branchName} is built on them.`
        : `It checks ${counted} too: ${joinNames(ancestors)}, which ${branchName} is built on.`
    return { lead: null, note }
  }
  if (root !== undefined) {
    const approvals =
      pullRequest === null
        ? ""
        : ` The approvals of pull request #${pullRequest} don't count: it compares ${branchName} with ${ancestors.at(-1)}, and Patronus merges a pull request into its base.`
    const lands = counted === null ? `${joinNames(ancestors)} with it` : `${counted} with it: ${joinNames(ancestors)}`
    return { lead: stackLead(branchName, target, ancestors, root, independence), note: `Or start ${branchName} now: the Safe Push lands ${lands}.${approvals}` }
  }
  if (children.length > 0) {
    const verb = children.length === 1 ? "is" : "are"
    const pullRequests = childrenHavePullRequests ? `; their pull requests are retargeted to ${target} when they are pushed` : ""
    return { lead: null, note: `${joinNames(children)} ${verb} built on ${branchName}: after the merge, the next sync moves them onto ${target}${pullRequests}.` }
  }
  return { lead: null, note: null }
}
