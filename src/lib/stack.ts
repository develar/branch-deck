import type { ParentState } from "@/lib/bindings"
import { formatBranchCount } from "@/lib/branch"
import { groupFor } from "@/lib/commit"
import { dropsRemoteCommits, getPushAction } from "@/lib/push-summary"
import { joinNames } from "@/lib/text"
import type { Branch } from "@/stores/branch-sync"

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

  const stored = stackParents[to.branch]
  if (stacks && to.parentSource === "prefix" && stored !== undefined && stored !== parent) {
    return { text: `${to.branch} is stacked on ${stored} in the stack parents: the prefix would conflict with it`, warning: true }
  }
  const target = branchOf(to.branch)
  // the other commits of the target branch keep declaring their parent
  const othersInTarget = (target?.commitCount ?? 0) - (sameBranch ? 1 : 0)
  if (stacks && to.origin === "prefix" && othersInTarget > 0 && (target?.declaredParent ?? null) !== parent) {
    const declared = target?.declaredParent ?? null
    return { text: `The other commits of ${to.branch} ${declared === null ? "declare no parent" : `declare ${declared}`}: its parents would conflict`, warning: true }
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

/** Whether a push of the branch has work for the remote; false while its remote status is unknown */
export function needsPush(branch: Branch): boolean {
  const action = getPushAction(branch)
  return action !== null && action !== "upToDate" && branch.status !== "Blocked" && !branch.hasError
}

/**
 * The branches a stacked branch is built on, root first: the chain of `effectiveBase` in the branch table (an ancestor
 * without commits is not on it, the branch is built past it). `branchOf` looks a branch up.
 */
export function stackAncestors(name: string, branchOf: (name: string) => Branch | undefined): Branch[] {
  const ancestors: Branch[] = []
  const seen = new Set<string>([name])
  let parentName = branchOf(name)?.stack?.effectiveBase ?? null
  while (parentName !== null && !seen.has(parentName)) {
    seen.add(parentName)
    const parent = branchOf(parentName)
    if (parent === undefined) {
      break
    }
    ancestors.push(parent)
    parentName = parent.stack?.effectiveBase ?? null
  }
  return ancestors.reverse()
}

/**
 * The branches a "Push Stack" of `name` pushes, root first: its ancestors in the branch table that have work for the
 * remote, then the branch itself. `branchOf` looks a branch up. With `stopAtConfirm` (the push button), the order starts
 * above the nearest ancestor whose push drops remote commits: that one asks first in its own row, and a stack pushed
 * around it would leave a stale branch in the middle. It always starts above the nearest ancestor a Safe Push holds
 * (`isHeld`, `PushHold`): a push of it won't land, and the robot holds every ancestor below it too.
 */
export function stackPushOrder(
  name: string,
  branchOf: (name: string) => Branch | undefined,
  { stopAtConfirm = false, isHeld = () => false }: { stopAtConfirm?: boolean; isHeld?: (name: string) => boolean } = {},
): string[] {
  const chain = [name]
  for (const ancestor of stackAncestors(name, branchOf).reverse()) {
    if (isHeld(ancestor.name) || (stopAtConfirm && ancestor.remoteStatus != null && needsPush(ancestor) && dropsRemoteCommits(ancestor.remoteStatus))) {
      break
    }
    if (needsPush(ancestor)) {
      chain.push(ancestor.name)
    }
  }
  return chain.reverse()
}

/**
 * The branches the push button of `name` pushes when it pushes the stack, root first (`stackPushOrder` with
 * `stopAtConfirm`): only while the branch has work for the remote, its own push asks nothing (a push that drops remote
 * commits confirms the branch alone) and an ancestor has work too; else an empty list, the button pushes the branch alone
 */
export function buttonStackPushOrder(name: string, branchOf: (name: string) => Branch | undefined, isHeld: (name: string) => boolean = () => false): string[] {
  const branch = branchOf(name)
  if (branch?.remoteStatus == null || !needsPush(branch) || dropsRemoteCommits(branch.remoteStatus)) {
    return []
  }
  const order = stackPushOrder(name, branchOf, { stopAtConfirm: true, isHeld })
  return order.length > 1 ? order : []
}

/**
 * The branches a "Push with Children" of `name` pushes, parents first: the branch if it has work for the remote, then
 * its `descendants` (parents before children) that have work. A child whose push drops remote commits is left out: it
 * asks first in its own row. A branch a Safe Push holds (`isHeld`) is left out too: its push won't land; a held branch
 * holds its ancestors, so the children of a branch that isn't held are held only by a race.
 */
export function stackChildrenPushOrder(
  name: string,
  descendants: readonly string[],
  branchOf: (name: string) => Branch | undefined,
  isHeld: (name: string) => boolean = () => false,
): string[] {
  const order: string[] = []
  const branch = branchOf(name)
  if (branch !== undefined && needsPush(branch) && !isHeld(name)) {
    order.push(name)
  }
  for (const descendant of descendants) {
    const child = branchOf(descendant)
    if (child?.remoteStatus != null && needsPush(child) && !dropsRemoteCommits(child.remoteStatus) && !isHeld(descendant)) {
      order.push(descendant)
    }
  }
  return order
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
 * (`stackChildrenPushOrder`).
 */
export function formatPushWithChildrenLabel(order: readonly string[]): string {
  return `Push with Children (${order.length === 1 ? order[0] : formatBranchCount(order.length)})`
}

/** Push with Children pushes more than the branch itself: a child has work; else the push of the branch is the push */
export function offersPushWithChildren(name: string, order: readonly string[]): boolean {
  return order.some(it => it !== name)
}

/**
 * The branches of `names` (the ones stacked on a branch) whose push has work but drops remote commits: Push with Children
 * leaves them out, they ask first in their own rows. A branch a running Safe Push holds (`isHeld`) has nothing to push.
 */
export function stackAskFirst(names: readonly string[], branchOf: (name: string) => Branch | undefined, isHeld: (name: string) => boolean = () => false): string[] {
  return names.filter(name => {
    const branch = branchOf(name)
    return branch?.remoteStatus != null && needsPush(branch) && dropsRemoteCommits(branch.remoteStatus) && !isHeld(name)
  })
}

/** "login asks first in its row: the push drops remote commits", "login and oauth ask first in their rows: …" */
export function formatAskFirst(names: readonly string[]): string {
  const one = names.length === 1
  return `${joinNames(names)} ${one ? "asks" : "ask"} first in ${one ? "its row" : "their rows"}: the push drops remote commits`
}

/**
 * Why Push with Children of `name` has nothing to push beyond the branch itself, as its menu item and the push of the
 * island header say it; null while it does (`offersPushWithChildren`). `order` is what it pushes
 * (`stackChildrenPushOrder`), `askFirst` the children left out because their push drops remote commits (`stackAskFirst`).
 */
export function describePushWithChildrenBlocker(name: string, order: readonly string[], askFirst: readonly string[]): string | null {
  if (offersPushWithChildren(name, order)) {
    return null
  }
  if (askFirst.length > 0) {
    return formatAskFirst(askFirst)
  }
  return order.includes(name) ? `Only ${name} has commits to push: its own push sends them` : "Nothing to push: the stack is on the remote"
}

/**
 * The toast action that unstacks a branch of a move: "Unstack from r" for the target of the move, which the toast
 * names, else "Unstack a from r", so the action says which branch it changes
 */
export function formatMoveUnstackAction(branchName: string, parent: string, moveTarget: string | null): string {
  return branchName === moveTarget ? `Unstack from ${parent}` : `Unstack ${branchName} from ${parent}`
}
