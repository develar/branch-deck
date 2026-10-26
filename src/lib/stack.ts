import type { ParentState, StackFix, StackProblemView } from "@/lib/bindings"
import { formatBranchCount } from "@/lib/branch"
import { groupFor } from "@/lib/commit"
import { joinNames } from "@/lib/text"
import type { Branch } from "@/stores/branch-sync"

/**
 * The stack problem the sync reports for the branch, the first of its problems, with its label, its reason, its message
 * and its fixes as Rust decides them (`StackProblemView`, the rule `bd` prints too); null for a branch without one
 */
export function stackProblemOf(branch: Branch): StackProblemView | null {
  return branch.stack?.problems[0] ?? null
}

/** The parents a fix of the problem stacks the branch on, in the order of the fixes */
export function stackOnFixes(problem: StackProblemView): string[] {
  return problem.fixes.flatMap(fix => (typeof fix === "object" ? [fix.StackOn] : []))
}

/** Whether one of the fixes of the problem is `fix` */
export function hasStackFix(problem: StackProblemView, fix: Exclude<StackFix, { StackOn: string }>): boolean {
  return problem.fixes.includes(fix)
}

/** The branch that keeps a blocked branch from being rebuilt (the parent, or the ancestor that can't be built); null if not blocked */
export function blockingBranchOf(branch: Branch): string | null {
  if (branch.status !== "Blocked") {
    return null
  }
  const stack = branch.stack
  return stack?.blockedBy?.parent ?? stack?.effectiveBase ?? stack?.declaredParent ?? null
}

/** Why a blocked branch can't be pushed nor given to Patronus: "Blocked by auth: fix it first, then sync" */
export function formatBlockedReason(branch: Branch): string {
  const blocker = blockingBranchOf(branch)
  return blocker === null ? "Blocked: fix the parent branch first" : `Blocked by ${blocker}: fix it first, then sync`
}

/** What the store knows about a branch for `describePrefixChange` */
export interface StackBranchInfo {
  /** The parent its commits declare, else its stack parent */
  declaredParent: string | null
  commitCount: number
}

export interface PrefixChange {
  text: string
  /** The change moves or restacks the commit, or makes a stack problem: shown as a warning */
  warning: boolean
  /**
   * The text for an action whose core refuses the stack problem the change makes (the reword is a regroup, which
   * refuses a problem it introduces); set only for such a problem
   */
  refused?: string
}

/**
 * What saving `after` instead of `before` does to the commit's place: same branch, another branch, another parent.
 * With `stacks`, it also warns about a parent that has no commits (the branch is built on `baseline` then, the baseline
 * as the remote names it, likely a typo), about a parent level that differs from the stack parent of the branch, and
 * about a parent that differs from the one the other commits of the branch declare. `branchOf` looks a branch up in the
 * branch table; `issueGroups` and `stackParents` are the maps of the last grouping.
 */
export function describePrefixChange(
  before: string,
  after: string,
  action: string,
  stacks: boolean,
  baseline: string,
  branchOf: (name: string) => StackBranchInfo | undefined,
  issueGroups: Readonly<Record<string, string>> = {},
  stackParents: Readonly<Record<string, string>> = {},
): PrefixChange {
  const from = groupFor(before, stacks, issueGroups, stackParents)
  const to = groupFor(after, stacks, issueGroups, stackParents)
  if (to?.invalid === true) {
    return { text: `The prefix has an empty level: ${to.branch} will report an error`, warning: true, refused: `${action} is refused: the prefix has an empty level` }
  }
  const sameBranch = (to?.branch ?? null) === (from?.branch ?? null)
  const parent = to?.parent ?? null
  if (sameBranch && (!stacks || parent === (from?.parent ?? null))) {
    return { text: "The first line is the subject", warning: false }
  }
  if (to === null) {
    return { text: `${action} moves the commit to the unassigned commits`, warning: true }
  }

  const stored = stackParents[to.branch]
  if (stacks && to.parentSource === "prefix" && stored !== undefined && stored !== parent) {
    return { text: `${to.branch} is stacked on ${stored} in the stack parents: the prefix would conflict with it`, warning: true }
  }
  const target = branchOf(to.branch)
  // the other commits of the target branch keep declaring their parent
  const othersInTarget = (target?.commitCount ?? 0) - (sameBranch ? 1 : 0)
  if (stacks && to.origin === "prefix" && othersInTarget > 0 && (target?.declaredParent ?? null) !== parent) {
    const declared = target?.declaredParent ?? null
    const others = `of ${to.branch} ${declared === null ? "declare no parent" : `declare ${declared}`}`
    return {
      text: `The other commits ${others}: its parents would conflict`,
      warning: true,
      refused: `${action} is refused: the other commits ${others}, its parents would conflict`,
    }
  }
  if (stacks && parent !== null && branchOf(parent) === undefined) {
    return { text: `${parent} has no commits: ${to.branch} is built on ${baseline} until it has`, warning: true }
  }
  const onParent = stacks && parent !== null ? ` on ${parent}` : ""
  if (sameBranch) {
    return { text: parent === null ? `${action} unstacks ${to.branch}` : `${action} stacks ${to.branch}${onParent}`, warning: true }
  }
  return { text: `${action} moves the commit to the branch ${to.branch}${onParent}`, warning: true }
}

/**
 * The branch name a remote knows the baseline by: `origin/master` is `master` there. Every pull request of a virtual
 * branch goes into it (`getPullRequestBase`).
 */
export function baselineOnRemote(baselineBranch: string): string {
  const slash = baselineBranch.indexOf("/")
  return slash === -1 ? baselineBranch : baselineBranch.slice(slash + 1)
}

/**
 * Why a branch isn't built on its declared parent, which has no commits: "air-misc landed", "air-misc is unapplied",
 * "air-misc has no commits"
 */
export function describeParentState(parent: string, state: Exclude<ParentState, "Active">): string {
  switch (state) {
    case "Integrated":
      return `${parent} landed`
    case "Unapplied":
      return `${parent} is unapplied`
    case "Missing":
      return `${parent} has no commits`
  }
}

/**
 * The declared parent of a branch built on another branch or the baseline because the parent has no commits, and how
 * the parent stands; null for any other branch
 */
export function passedParentOf(branch: Branch): { parent: string; state: Exclude<ParentState, "Active"> } | null {
  const stack = branch.stack
  const parent = stack?.declaredParent ?? null
  const state = stack?.parentState ?? null
  if (stack?.prBaseChanged !== true || parent === null || state === null || state === "Active") {
    return null
  }
  return { parent, state }
}

/**
 * What a stack push started from `startedFrom` sends, as the texts of its push name it while it runs (`stackPushes`):
 * "7 branches" for its one push of several, the one branch it pushes, `startedFrom` while it waits for a sync
 */
export function stackPushSubject(startedFrom: string, names: readonly string[] | null): string {
  if (names === null || names.length === 0) {
    return startedFrom
  }
  return names.length === 1 ? names[0]! : formatBranchCount(names.length)
}

/** "Pushing", "Pushing login", "Pushing 7 branches": the push of `startedFrom` while its stack push runs (`stackPushSubject`) */
export function formatStackPushing(startedFrom: string, names: readonly string[] | null): string {
  const subject = stackPushSubject(startedFrom, names)
  return subject === startedFrom ? "Pushing" : `Pushing ${subject}`
}

/** "Push Stack (3 branches)": the push button, its description and the branch menu name the action alike */
export function formatPushStackLabel(branchCount: number): string {
  return `Push Stack (${formatBranchCount(branchCount)})`
}

/**
 * "Push with Children (3 branches)", "Push with Children (oauth)" for a push of one child: the branch menu, the
 * description of the push button and the stack header name the action alike. `order` is what it pushes
 * (`ChildrenPushOrder.order`); `withStackBranch`: the stack branch of the whole stack goes along (`StackBranchAlong`),
 * counted as one more branch, "Push with Children (the whole stack branch)" alone.
 */
export function formatPushWithChildrenLabel(order: readonly string[], withStackBranch = false): string {
  const count = order.length + (withStackBranch ? 1 : 0)
  if (count === 1) {
    return `Push with Children (${order[0] ?? "the whole stack branch"})`
  }
  return `Push with Children (${formatBranchCount(count)})`
}

/** "login asks first in its row: the push drops remote commits", "login and oauth ask first in their rows: …" */
export function formatAskFirst(names: readonly string[]): string {
  const one = names.length === 1
  return `${joinNames(names)} ${one ? "asks" : "ask"} first in ${one ? "its row" : "their rows"}: the push drops remote commits`
}

/**
 * The toast action that unstacks a branch of a move: "Unstack from r" for the target of the move, which the toast
 * names, else "Unstack a from r", so the action says which branch it changes
 */
export function formatMoveUnstackAction(branchName: string, parent: string, moveTarget: string | null): string {
  return branchName === moveTarget ? `Unstack from ${parent}` : `Unstack ${branchName} from ${parent}`
}
