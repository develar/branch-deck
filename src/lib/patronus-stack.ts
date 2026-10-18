import type { PatronusPushMode, StackCommit, StackIndependence, StackUse } from "@/lib/bindings"
import { formatBranchesAndCommits } from "@/lib/branch"
import { formatCommitCount } from "@/lib/commit"
import { joinNames } from "@/lib/text"

/**
 * The second way to land a stacked branch, never the lead: alone, after the unstack from its parent (it builds without
 * it), or once the commits it needs moved into it. `warning`: the branch applies alone but uses a name of its stack;
 * names, not a compiler, found the use, so the unstack is offered, never decided
 */
export type StackAlone =
  | { readonly kind: "unstack"; readonly parent: string; readonly text: string; readonly warning: boolean }
  | { readonly kind: "move"; readonly commits: readonly StackCommit[]; readonly label: string; readonly text: string }

/** What a Safe Push or a Dry Run of a branch lands, and what it does to the rest of its stack (`describeStackLanding`) */
export interface StackLanding {
  /** The one robot lands the branch with what it is built on: "Lands oauth with auth and login, …"; null for a root */
  readonly lands: string | null
  /** The branches built on it stay: what happens to them after the merge; null without any or for a dry run */
  readonly children: string | null
  /** To land the branch alone (`StackAlone`); null for a root, a dry run, before the trial build answers or when it can't */
  readonly alone: StackAlone | null
  /** Its open pull request merges into a virtual branch: the approvals count once it goes into the target */
  readonly retarget: { readonly number: number; readonly text: string; readonly label: string } | null
}

export interface StackLandingInput {
  readonly branchName: string
  readonly mode: PatronusPushMode
  /** The target branch of Patronus ("master") */
  readonly target: string
  /** The branches the branch is built on, root first (`stackAncestors`) */
  readonly ancestors: readonly string[]
  /** The commits the robot lands: those of the branch and of its ancestors */
  readonly commitCount: number
  /** The branches built on the branch (`stackDescendantsIn`) */
  readonly children: readonly string[]
  /** The open pull request of the branch into a virtual branch (`{prefix}/virtual/…`); null without one */
  readonly virtualBase: { readonly number: number; readonly base: string } | null
  /** The trial build of the branch alone (`get_stack_independence`); null before it answers, when it failed, or for a root */
  readonly independence: StackIndependence | null
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
 * The branch alone, by the trial build: it builds without its parent (the unstack, a warning when it uses a name of the
 * stack), or it needs commits that can move into it (the move, then the check again). The checks are lines and names,
 * not a compiler, so each is offered, never done. Needed commits that can't move, or a conflict: no way alone.
 */
function describeAlone(branchName: string, independence: StackIndependence): StackAlone | null {
  if (independence.kind === "builds") {
    const { parent, uses, symbols } = independence
    if (uses.length > 0) {
      const text = `${describeUses(uses)} ${branchName} applies on the baseline without ${parent}, but needs it to compile. Found by names, not by a compiler.`
      return { kind: "unstack", parent, text, warning: true }
    }
    // the trial build is on the base the sync builds on, not on the newest target
    const checked = symbols ? " and uses no name it adds. Not checked by a compiler." : ". Compile dependencies are not checked."
    return { kind: "unstack", parent, text: `To land ${branchName} alone: it applies on the baseline without ${parent}${checked}`, warning: false }
  }
  const { childCommit, parentCommits, movable } = independence
  if (!movable || parentCommits.length === 0) {
    return null
  }
  const count = parentCommits.length
  const label = count === 1 ? "Move 1 Commit Here" : `Move ${count} Commits Here`
  return { kind: "move", commits: parentCommits, label, text: `To land ${branchName} alone, move what it needs here: ${describeNeed(childCommit, parentCommits)}` }
}

/**
 * A stacked branch carries the commits of the branches it is built on, so its Safe Push lands them with it, in one
 * robot: "Lands oauth with auth and login, which it is built on: 3 branches · 12 commits into master." (the names cut
 * from three ancestors on: "a, b, c and 2 more"). A dry run checks them the same way. The branches built on it stay, and
 * the next sync after the merge builds them on the target. Landing the branch alone is the second way, by the trial
 * build (`describeAlone`); an open pull request into a virtual branch is retargeted to the target first, so its
 * approvals count.
 */
export function describeStackLanding({ branchName, mode, target, ancestors, commitCount, children, virtualBase, independence }: StackLandingInput): StackLanding {
  const builtOn = `${joinNames(ancestors)}, which it is built on: ${formatBranchesAndCommits(ancestors.length + 1, commitCount)}`
  if (mode === "dryRun") {
    return { lands: ancestors.length === 0 ? null : `Checks ${branchName} with ${builtOn}.`, children: null, alone: null, retarget: null }
  }
  const one = children.length === 1
  const childrenNote =
    children.length === 0
      ? null
      : `${joinNames(children)} ${one ? "is" : "are"} built on ${branchName}: ${one ? "it stays" : "they stay"}, and after the merge the next sync builds ${one ? "it" : "them"} on ${target}.`
  const retarget =
    virtualBase === null
      ? null
      : {
          number: virtualBase.number,
          text: `Approvals in #${virtualBase.number} don't count: it merges into ${virtualBase.base}, and Patronus merges a pull request into its base.`,
          label: `Retarget #${virtualBase.number} to ${target}`,
        }
  return {
    lands: ancestors.length === 0 ? null : `Lands ${branchName} with ${builtOn} into ${target}.`,
    children: childrenNote,
    alone: ancestors.length === 0 || independence === null ? null : describeAlone(branchName, independence),
    retarget,
  }
}
