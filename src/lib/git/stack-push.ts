import { Channel } from "@tauri-apps/api/core"
import type { PatronusRobot, TransferProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { Retargeted } from "@/lib/git/new-pull-request"
import { retargetBeforePush } from "@/lib/git/new-pull-request"
import type { PushReport } from "@/lib/git/push"
import { cancelHoldingRobot, checkPushHold, isHeldBy, pushesOverMovedRemote, restartAfterPush, sentPart, showRemoteMovedToast } from "@/lib/git/push"
import type { PushOutcome } from "@/lib/git/push-toast"
import { showStackPushToast } from "@/lib/git/push-toast"
import { formatPushMode } from "@/lib/patronus"
import { showRobotToast } from "@/lib/patronus-actions"
import type { PushUpToStep } from "@/lib/push-up-to"
import { stackChildrenPushOrder, stackPushOrder } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import { formatWholeStack, getStackBranchBlocker } from "@/lib/whole-stack"
import { recordPush } from "@/stores/activity"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import {
  branchSyncStore,
  getBranch,
  getStackBranch,
  getStackDescendants,
  isBranchPushing,
  isStackPushing,
  setBranchesPushProgress,
  setBranchPushing,
  setBranchRemoteStatus,
  setStackBranchRemoteStatus,
  setStackPush,
  toRemoteStatus,
  whenBranchPushed,
  whenSynced,
} from "@/stores/branch-sync"
import { getPushHold, getStackPushHold } from "@/stores/patronus"
import { refreshPullRequests, refreshPullRequestsAfterPushes } from "@/stores/pull-requests"
import { vcsRequestFactory } from "@/stores/repository"

export interface StackPushOptions {
  /** The order of the push button: it starts above an ancestor whose push drops remote commits (`stackPushOrder`) */
  stopAtConfirm?: boolean
  /** The push is a step of a Safe Push start: the robot it replaces is canceled, never restarted (`PushOptions`) */
  replacesRobot?: PatronusRobot
  /** Whether a toast says what was pushed (`PushOptions`) */
  announce?: boolean
  /** Where a failure is said (`PushOptions`) */
  report?: PushReport
}

/** What a stack push sends: the branches, parents first, and the ones pushed up to a commit (Push Up to Here) */
interface StackPushOrder {
  names: readonly string[]
  tips?: ReadonlyMap<string, string>
}

/**
 * Push Stack: the ancestors of a stacked branch that have work for the remote, root first, then the branch, in one
 * atomic push (`pushTogether`). Resolves to whether they were pushed.
 */
export async function pushStack(branchName: string, options: StackPushOptions = {}): Promise<boolean> {
  return await pushTogether(
    branchName,
    () => ({ names: stackPushOrder(branchName, getBranch, { stopAtConfirm: options.stopAtConfirm, isHeld: name => isHeldBy(name, options.replacesRobot) }) }),
    options,
  )
}

/**
 * Push with Children: the branch if it has work for the remote, then the branches stacked on it that have work, parents
 * first, in one atomic push (`pushTogether`)
 */
export async function pushWithChildren(branchName: string): Promise<boolean> {
  return await pushTogether(branchName, () => ({ names: stackChildrenPushOrder(branchName, getStackDescendants(branchName), getBranch, name => isHeldBy(name, undefined)) }), {})
}

/**
 * Push Up to Here (`pushUpToPlan`): the branches the commit is built on that have work for the remote, whole, root
 * first, then its branch up to it, in one atomic push (`pushTogether`)
 */
export async function pushUpTo(steps: readonly PushUpToStep[]): Promise<boolean> {
  const last = steps.at(-1)
  if (last === undefined) {
    return false
  }
  const tips = new Map(steps.flatMap(step => (step.tip === undefined ? [] : [[step.name, step.tip] as const])))
  return await pushTogether(last.name, () => ({ names: steps.map(step => step.name), tips }), {})
}

/**
 * Pushes the branches `orderOf` names, parents first, in one atomic push (`push_branches`): all of them or none. A sync
 * that runs is awaited first, so the order is computed on the branches as the sync left them. The holds and the
 * retargets are each branch's own, then the one push runs while every row of it shows one progress, and the row of
 * `startedFrom` names it ("Pushing 7 branches 45%"); one toast says what was pushed (`showStackPushToast`), Activity has an
 * entry per branch. A remote branch that moved since its row showed it refuses the whole push: the branches are
 * fetched, and the push goes again over what the remotes have now, once, when that remote has no commits its branch
 * lacks (`pushesOverMovedRemote`), else its toast says what it has. A second push from the same row while one runs does
 * nothing.
 * Resolves to whether the branches were pushed; true when nothing is left to push.
 */
async function pushTogether(startedFrom: string, orderOf: () => StackPushOrder, options: StackPushOptions): Promise<boolean> {
  if (isStackPushing(startedFrom)) {
    return false
  }
  const report = options.report ?? notifyError
  // the row shows the push while it waits for a sync, and a second click does nothing
  setStackPush(startedFrom, [])
  try {
    const order = await settledOrder(orderOf)
    if (order.names.length === 0 || !(await checkStackHolds(order.names, options, report))) {
      return order.names.length === 0
    }
    // the checks may have waited for Patronus while a sync changed the branches: push them as they are now
    const branches = findBranches(order.names, report)
    if (branches === null) {
      return false
    }
    const tips = order.tips ?? new Map<string, string>()
    // a pull request into a virtual branch goes into the baseline first, with what it lands
    const retargeted = new Map<string, Retargeted>()
    for (const branch of branches) {
      const it = await retargetBeforePush(branch)
      if (it !== null) {
        retargeted.set(branch.name, it)
      }
    }
    setStackPush(startedFrom, order.names)
    let pushed = branches
    let moved = await sendTogether(pushed, tips, report)
    // nobody's work is lost: the stack goes over what the remotes have now, once, as the push of one branch does; a
    // remote that moves again, or one with commits its branch lacks, is told
    if (moved != null && pushesOverMovedRemote(moved.status)) {
      const fetched = findBranches(order.names, report)
      if (fetched === null) {
        return false
      }
      pushed = fetched
      moved = await sendTogether(pushed, tips, report)
    }
    if (moved === undefined) {
      return false
    }
    if (moved !== null) {
      await showRemoteMovedToast(moved.name, moved.status, undefined)
      return false
    }
    if (options.announce !== false) {
      const outcomes: PushOutcome[] = pushed.map(branch => ({
        name: branch.name,
        before: sentPart(branch.remoteStatus, tips.get(branch.name)),
        retargeted: retargeted.get(branch.name),
      }))
      showStackPushToast(outcomes, startedFrom)
    }
    return true
  } finally {
    setStackPush(startedFrom, null)
  }
}

/**
 * The order once no sync runs and no push of one of its branches from another row: a stack push waits for them and goes
 * on the branches as they leave them
 */
async function settledOrder(orderOf: () => StackPushOrder): Promise<StackPushOrder> {
  while (true) {
    await whenSynced()
    const order = orderOf()
    const running = order.names.filter(name => isBranchPushing(name))
    if (running.length === 0) {
      return order
    }
    await Promise.all(running.map(name => whenBranchPushed(name)))
  }
}

/**
 * Whether no running Safe Push holds a branch of the push (`checkPushHold`), so the push may go: one held branch
 * refuses the whole push, with the reason. The robot a Safe Push start replaces is canceled once, at the first branch it
 * holds.
 */
async function checkStackHolds(names: readonly string[], options: StackPushOptions, report: PushReport): Promise<boolean> {
  let canceled: string | null = null
  for (const name of names) {
    const hold = getPushHold(name)
    if (hold !== null && hold.robot.id === canceled) {
      continue
    }
    if (!(await checkPushHold(name, { replacesRobot: options.replacesRobot }, report))) {
      return false
    }
    if (hold !== null && hold.robot.id === options.replacesRobot?.id) {
      canceled = hold.robot.id
    }
  }
  return true
}

/** The branches in the table, or null when one is gone (it said so) */
function findBranches(names: readonly string[], report: PushReport): Branch[] | null {
  const branches: Branch[] = []
  for (const name of names) {
    const branch = getBranch(name)
    if (branch === undefined) {
      report(`Cannot push ${name}`, `${name} is no longer in the branch list`)
      return null
    }
    branches.push(branch)
  }
  return branches
}

/**
 * The one push of the branches (`push_branches`), each leased on the remote head its row shows, while their rows show
 * its progress; their remote statuses are updated from the result. The branch whose remote moved when nothing was
 * pushed, with what its remote has now; null when they were pushed; undefined when the push failed (it said why).
 */
async function sendTogether(
  branches: readonly Branch[],
  tips: ReadonlyMap<string, string>,
  report: PushReport,
): Promise<{ name: string; status: RemoteStatus } | null | undefined> {
  const names = branches.map(branch => branch.name)
  for (const name of names) {
    setBranchPushing(name, true)
  }
  try {
    const request = vcsRequestFactory.createRequest()
    const progress = new Channel<TransferProgress>()
    progress.onmessage = pushProgress => {
      setBranchesPushProgress(names, pushProgress)
    }
    const result = await commands.pushBranches(
      {
        repositoryPath: request.repositoryPath,
        branchPrefix: request.branchPrefix,
        myEmail: branches[0]?.myEmail ?? null,
        baselineBranch: branchSyncStore.getState().baselineBranch!,
        branches: branches.map(branch => ({
          branchName: branch.name,
          // a stacked branch's own commits are those after its parent
          parentBranch: branch.stack?.effectiveBase ?? null,
          // without a loaded status the branch must not be on the remote: if it is, the push fetches and shows it
          expectedRemote: branch.remoteStatus?.remoteTip ?? null,
          tip: tips.get(branch.name) ?? null,
        })),
      },
      progress,
    )
    if (result.status === "error") {
      report(`Cannot push ${joinNames(names)}`, result.error)
      return undefined
    }
    const statuses = result.data.remoteStatuses.map(status => toRemoteStatus(status))
    branches.forEach((branch, index) => {
      const status = statuses[index]
      if (status !== undefined) {
        setBranchRemoteStatus(branch.name, status)
      }
    })
    if (result.data.type === "remoteMoved") {
      const index = names.indexOf(result.data.branchName)
      const status = statuses[index]
      if (status === undefined) {
        report(`Cannot push ${joinNames(names)}`, `The remote branch of ${result.data.branchName} changed: nothing was pushed`)
        return undefined
      }
      recordPush(request.repositoryPath, result.data.branchName, branches[index]!.remoteStatus, status)
      return { name: result.data.branchName, status }
    }
    for (const branch of branches) {
      recordPush(request.repositoryPath, branch.name, sentPart(branch.remoteStatus, tips.get(branch.name)), null)
    }
    void refreshPullRequestsAfterPushes(branches.map(branch => [branch.name, tips.get(branch.name) ?? branch.commits[0]?.hash] as const))
    return null
  } catch (error) {
    report(`Cannot push ${joinNames(names)}`, error)
    return undefined
  } finally {
    for (const name of names) {
      setBranchPushing(name, false)
    }
  }
}

export interface WholeStackPushOptions {
  /**
   * The robot of the whole stack the push restarts on the pushed commit ("Restart Safe Push"): a running one is canceled
   * first, as a push into its stack branch would fail it at the end
   */
  restartRobot?: PatronusRobot
  /**
   * The robot of the whole stack a new start replaces (a Safe Push on an older stack branch): canceled first, as a push
   * into its stack branch would fail it at the end, and not restarted
   */
  replacesRobot?: PatronusRobot
  /** Where a failure is said (`PushOptions`) */
  report?: PushReport
}

/**
 * Pushes the stack branch of the whole stack of `root` (`{prefix}/stack/<root>`), leased on the remote head its status
 * showed, as one more branch of `push_branches` (`wholeStack`): the head of the pull request of the whole stack, the
 * source of its robot. Only a stack branch the sync built (`getStackBranchBlocker`); never while its own Safe Push runs
 * (`getStackPushHold`), unless the push restarts that robot (`restartRobot`, canceled first, restarted on the pushed
 * commit) or a new start replaces it (`replacesRobot`, canceled first). A sync that runs is awaited first. A remote branch that moved is fetched, and the push goes again over it
 * once when it has no commit the stack branch lacks (`pushesOverMovedRemote`). The branches of the stack stay as they
 * are on the remote. No toast: the action it is a step of says it; Activity records the push. Resolves to whether it
 * was pushed.
 */
export async function pushWholeStack(root: string, options: WholeStackPushOptions = {}): Promise<boolean> {
  const report = options.report ?? notifyError
  const subject = formatWholeStack(root)
  await whenSynced()
  const hold = getStackPushHold(root)
  if (hold !== null) {
    if (hold.robot.id !== options.restartRobot?.id && hold.robot.id !== options.replacesRobot?.id) {
      report(`Cannot push ${subject}`, `A ${formatPushMode(hold.robot.pushMode)} of ${subject} is running: wait until it merges, or restart it on the current stack`)
      return false
    }
    if (!(await cancelHoldingRobot(hold.robot, report))) {
      return false
    }
  }
  let moved = await sendStackBranch(root, report)
  if (moved !== null && moved !== undefined && pushesOverMovedRemote(moved)) {
    moved = await sendStackBranch(root, report)
  }
  if (moved === undefined) {
    return false
  }
  if (moved !== null) {
    report(
      `Cannot push ${subject}`,
      `Its remote branch has ${moved.commitsBehind === 1 ? "a commit" : `${moved.commitsBehind} commits`} the stack branch lacks: sync, then push again`,
    )
    return false
  }
  if (options.restartRobot !== undefined) {
    const restarted = await restartAfterPush(options.restartRobot)
    // the restart is the action: its toast is the one toast, Activity has the push and the restart
    if (restarted !== undefined) {
      showRobotToast(`${formatPushMode(restarted.pushMode)} restarted`, restarted)
    }
  }
  return true
}

/**
 * The one push of the stack branch, leased on the remote head of its status; its remote status is updated from the
 * result. What its remote has now when it moved, null when it was pushed, undefined when the push failed (it said why).
 */
async function sendStackBranch(root: string, report: PushReport): Promise<RemoteStatus | null | undefined> {
  const subject = formatWholeStack(root)
  const stack = getStackBranch(root)
  const blocker = getStackBranchBlocker(root, stack)
  if (stack === undefined || blocker !== null) {
    report(`Cannot push ${subject}`, blocker ?? "The stack has no stack branch")
    return undefined
  }
  const request = vcsRequestFactory.createRequest()
  try {
    const result = await commands.pushBranches(
      {
        repositoryPath: request.repositoryPath,
        branchPrefix: request.branchPrefix,
        myEmail: getBranch(root)?.myEmail ?? null,
        baselineBranch: branchSyncStore.getState().baselineBranch!,
        branches: [{ branchName: root, wholeStack: true, expectedRemote: stack.remoteStatus?.remoteTip ?? null }],
      },
      new Channel<TransferProgress>(),
    )
    if (result.status === "error") {
      report(`Cannot push ${subject}`, result.error)
      return undefined
    }
    const status = result.data.remoteStatuses[0] === undefined ? null : toRemoteStatus(result.data.remoteStatuses[0])
    if (status !== null) {
      setStackBranchRemoteStatus(root, status)
    }
    recordPush(request.repositoryPath, subject, stack.remoteStatus, result.data.type === "remoteMoved" ? status : null)
    if (result.data.type === "remoteMoved") {
      return status ?? undefined
    }
    void refreshPullRequests("push")
    return null
  } catch (error) {
    report(`Cannot push ${subject}`, error)
    return undefined
  }
}
