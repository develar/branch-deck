import { groupFor } from "@/lib/commit"
import { getPushAction } from "@/lib/push-summary"
import type { Branch } from "@/stores/branch-sync"

/** The branch that keeps a blocked branch from being rebuilt (the parent, or the ancestor that is missing); null if not blocked */
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
  /** The parent its commits declare */
  declaredParent: string | null
  commitCount: number
}

export interface PrefixChange {
  text: string
  /** The change moves or restacks the commit, or makes a stack problem: shown as a warning */
  warning: boolean
}

/**
 * What saving `after` instead of `before` does to the commit's place: same branch, another branch, another parent.
 * With `stacks`, it also warns about a parent that has no commits and about a parent that differs from the one the
 * other commits of the branch declare. `branchOf` looks a branch up in the branch table.
 */
export function describePrefixChange(before: string, after: string, action: string, stacks: boolean, branchOf: (name: string) => StackBranchInfo | undefined): PrefixChange {
  const from = groupFor(before, stacks)
  const to = groupFor(after, stacks)
  if (to?.invalid === true) {
    return { text: `The prefix has an empty level: ${to.branch} will report an error`, warning: true }
  }
  const sameBranch = (to?.branch ?? null) === (from?.branch ?? null)
  const parent = to?.parent ?? null
  if (sameBranch && (!stacks || parent === (from?.parent ?? null))) {
    return { text: "The first line is the subject", warning: false }
  }
  if (to === null) {
    return { text: `${action} moves the commit to the unassigned commits`, warning: true }
  }

  const target = branchOf(to.branch)
  // the other commits of the target branch keep declaring their parent
  const othersInTarget = (target?.commitCount ?? 0) - (sameBranch ? 1 : 0)
  if (stacks && to.origin === "prefix" && othersInTarget > 0 && (target?.declaredParent ?? null) !== parent) {
    const declared = target?.declaredParent ?? null
    return { text: `The other commits of ${to.branch} ${declared === null ? "declare no parent" : `declare ${declared}`}: its parents would conflict`, warning: true }
  }
  if (stacks && parent !== null && branchOf(parent) === undefined) {
    return { text: `${parent} has no commits: ${to.branch} would be blocked until it has`, warning: true }
  }
  const onParent = stacks && parent !== null ? ` on ${parent}` : ""
  if (sameBranch) {
    return { text: parent === null ? `${action} unstacks ${to.branch}` : `${action} stacks ${to.branch}${onParent}`, warning: true }
  }
  return { text: `${action} moves the commit to the branch ${to.branch}${onParent}`, warning: true }
}

/** The branch name a remote knows the baseline by: `origin/master` is `master` there */
function baselineOnRemote(baselineBranch: string): string {
  const slash = baselineBranch.indexOf("/")
  return slash === -1 ? baselineBranch : baselineBranch.slice(slash + 1)
}

/**
 * The base of the pull request of a branch: the full branch name of the branch it is built on, or the baseline.
 * `fullName` turns a virtual branch name into the pushed one.
 */
export function pullRequestBaseOf(branch: Branch, baselineBranch: string, fullName: (name: string) => string): string {
  const base = branch.stack?.effectiveBase ?? null
  return base === null ? baselineOnRemote(baselineBranch) : fullName(base)
}

/** Whether a push of the branch has work for the remote; false while its remote status is unknown */
export function needsPush(branch: Branch): boolean {
  const action = getPushAction(branch)
  return action !== null && action !== "upToDate" && branch.status !== "Blocked" && !branch.hasError
}

/**
 * The branches a "Push Stack" of `name` pushes, root first: its ancestors in the branch table that have work for the
 * remote, then the branch itself. `branchOf` looks a branch up.
 */
export function stackPushOrder(name: string, branchOf: (name: string) => Branch | undefined): string[] {
  const chain: string[] = []
  const seen = new Set<string>()
  let current = branchOf(name)
  while (current !== undefined && !seen.has(current.name)) {
    seen.add(current.name)
    if (current.name === name || needsPush(current)) {
      chain.push(current.name)
    }
    const parent = current.stack?.effectiveBase ?? null
    current = parent === null ? undefined : branchOf(parent)
  }
  return chain.reverse()
}

/** "Push Stack (3 branches)": the branch menu and the description of the push button name the action alike */
export function formatPushStackLabel(branchCount: number): string {
  return `Push Stack (${branchCount} branches)`
}
