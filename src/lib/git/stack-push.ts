import { Channel } from "@tauri-apps/api/core"
import type { PatronusRobot, TransferProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { Retargeted } from "@/lib/git/new-pull-request"
import { retargetBeforePush } from "@/lib/git/new-pull-request"
import type { PushReport } from "@/lib/git/push"
import { cancelHoldingRobot, checkPushHold, createPushRequest, isHeldBy, isStillIn, pushesOverMovedRemote, restartAfterPush, sentPart, showRemoteMovedToast } from "@/lib/git/push"
import type { PushOutcome } from "@/lib/git/push-toast"
import { showStackPushToast } from "@/lib/git/push-toast"
import type { VcsRequest } from "@/lib/git/vcs-request"
import { formatPushMode, formatStackPushHoldReason } from "@/lib/patronus"
import { showRobotToast } from "@/lib/patronus-actions"
import { getPushAction } from "@/lib/push-summary"
import type { PushUpToStep } from "@/lib/push-up-to"
import { stackChildrenPushOrder, stackPushOrder } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import { formatWholeStack, formatWholeStackBranch, getStackBranchAlong, getStackBranchBlocker, stackReadinessKey } from "@/lib/whole-stack"
import { recordPush } from "@/stores/activity"
import type { Branch, RemoteStatus, StackBranch } from "@/stores/branch-sync"
import {
  branchSyncStore,
  getBranch,
  getStackBranch,
  getUnparkedStackDescendants,
  isBranchPushing,
  isStackPushing,
  setBranchesPushProgress,
  setBranchPushing,
  setBranchPushProgress,
  setBranchRemoteStatus,
  setStackBranchRemoteStatus,
  setStackPush,
  toRemoteStatus,
  whenBranchPushed,
  whenSynced,
} from "@/stores/branch-sync"
import { getPushHold, getStackBranchHoldReason, getStackLandedReason, getStackPushHold } from "@/stores/patronus"
import { getStackPullRequest, refreshPullRequests, refreshPullRequestsAfterPushes } from "@/stores/pull-requests"

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

/**
 * What a stack push sends: the branches, parents first, the ones pushed up to a commit (Push Up to Here), and the root
 * whose stack branch of the whole stack goes along after them (Push with Children of the root, `getStackBranchAlong`)
 */
interface StackPushOrder {
  names: readonly string[]
  tips?: ReadonlyMap<string, string>
  wholeStack?: string | null
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
 * first, and for the root of a stack that forks the stack branch of its whole stack (`takesStackBranchAlong`), in one
 * atomic push (`pushTogether`), so the pull request of the whole stack follows its branches
 */
export async function pushWithChildren(branchName: string): Promise<boolean> {
  return await pushTogether(
    branchName,
    () => ({
      names: stackChildrenPushOrder(branchName, getUnparkedStackDescendants(branchName), getBranch, name => isHeldBy(name, undefined)),
      wholeStack: takesStackBranchAlong(branchName) ? branchName : null,
    }),
    {},
  )
}

/**
 * Whether Push with Children of `root` takes the stack branch of its whole stack along now (`getStackBranchAlong`):
 * never while it is held (`getStackBranchHoldReason`): the robot of the whole stack runs, a push into its stack branch
 * would fail it, or every branch of the stack landed, a push of it lands nothing
 */
export function takesStackBranchAlong(root: string): boolean {
  return getStackBranchAlong(root, getStackBranch(root), getStackPullRequest(root), getStackBranchHoldReason(root))?.kind === "along"
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
 * entry per branch. The stack branch of a whole stack the order takes along (`wholeStack`) goes last, leased on the
 * remote head of its status, only while the sync built it. A remote branch that moved since its row showed it refuses
 * the whole push: the branches are fetched, and the push goes again over what the remotes have now, once, when that
 * remote has no commits its branch lacks (`pushesOverMovedRemote`), else its toast says what it has. A second push from
 * the same row while one runs does nothing.
 * Resolves to whether the branches were pushed; true when nothing is left to push.
 */
async function pushTogether(startedFrom: string, orderOf: () => StackPushOrder, options: StackPushOptions): Promise<boolean> {
  if (isStackPushing(startedFrom)) {
    return false
  }
  const report = options.report ?? notifyError
  const request = createPushRequest(`Cannot push ${startedFrom}`, report)
  if (request === null) {
    return false
  }
  // the branches of the table are of the selected repository: the push goes on only while it is the one it started in
  const stillIn = () => isStillIn(request.repositoryPath, startedFrom, report)
  // the row shows the push while it waits for a sync, and a second click does nothing
  setStackPush(startedFrom, [])
  // a push of the stack branch alone asked meanwhile (Request Review…, a restart) waits for this one, which may take it
  let settleStackBranch: (() => void) | null = null
  try {
    const order = await settledOrder(orderOf)
    if (!stillIn()) {
      return false
    }
    const wholeStack = order.wholeStack ?? null
    if (wholeStack !== null) {
      settleStackBranch = joinableStackBranchPush(wholeStack)
    }
    if ((order.names.length === 0 && wholeStack === null) || !(await checkStackHolds(order.names, options, report))) {
      return order.names.length === 0 && wholeStack === null
    }
    // the checks may have waited for Patronus while a sync changed the branches: push them as they are now
    if (!stillIn()) {
      return false
    }
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
    if (!stillIn()) {
      return false
    }
    setStackPush(startedFrom, order.names)
    let pushed = branches
    let stack = stackBranchAlong(wholeStack)
    let moved = await sendTogether(request, pushed, tips, stack, report)
    // nobody's work is lost: the stack goes over what the remotes have now, once, as the push of one branch does; a
    // remote that moves again, or one with commits its branch lacks, is told
    if (moved != null && pushesOverMovedRemote(moved.status)) {
      if (!stillIn()) {
        return false
      }
      const fetched = findBranches(order.names, report)
      if (fetched === null) {
        return false
      }
      pushed = fetched
      stack = stackBranchAlong(wholeStack)
      moved = await sendTogether(request, pushed, tips, stack, report)
    }
    if (moved === undefined) {
      return false
    }
    if (moved !== null) {
      if (moved.wholeStack) {
        report(`Cannot push ${formatWholeStackBranch(moved.name)}`, describeMovedStackBranch(moved.status))
      } else {
        await showRemoteMovedToast(moved.name, moved.status, undefined)
      }
      return false
    }
    if (options.announce !== false) {
      const outcomes: PushOutcome[] = pushed.map(branch => ({
        name: branch.name,
        before: sentPart(branch.remoteStatus, tips.get(branch.name)),
        retargeted: retargeted.get(branch.name),
      }))
      showStackPushToast(outcomes, startedFrom, stack === null ? null : { root: stack.root, before: stack.remoteStatus, pullRequest: getStackPullRequest(stack.root) })
    }
    return true
  } finally {
    settleStackBranch?.()
    setStackPush(startedFrom, null)
  }
}

/** The stack branch of the whole stack of `root` as the push sends it: only while it still goes along; null otherwise */
function stackBranchAlong(root: string | null): StackBranch | null {
  return root !== null && takesStackBranchAlong(root) ? (getStackBranch(root) ?? null) : null
}

/** Why a stack branch whose remote moved isn't pushed: its remote has commits it lacks, and the next sync rebuilds it */
function describeMovedStackBranch(status: RemoteStatus): string {
  return `Its remote branch has ${status.commitsBehind === 1 ? "a commit" : `${status.commitsBehind} commits`} the stack branch lacks: sync, then push again`
}

/**
 * The order once no sync runs and no push of one of its branches from another row, nor of the stack branch it takes
 * along (a request of it alone, its restart included): a stack push waits for them and goes on the branches as they
 * leave them, the stack branch along only while its remote still lacks it
 */
async function settledOrder(orderOf: () => StackPushOrder): Promise<StackPushOrder> {
  while (true) {
    await whenSynced()
    const order = orderOf()
    const names = order.wholeStack == null ? order.names : [...order.names, stackReadinessKey(order.wholeStack)]
    const running = names.filter(name => isBranchPushing(name))
    const stackPush = order.wholeStack == null ? undefined : runningStackBranchPushes.get(order.wholeStack)
    if (running.length === 0 && stackPush === undefined) {
      return order
    }
    await Promise.all([...running.map(name => whenBranchPushed(name)), stackPush?.done])
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
 * The one push of the branches (`push_branches`), each leased on the remote head its row shows, and the stack branch of
 * a whole stack after them (`stack`, leased on the remote head of its status), while their rows show its progress (the
 * stack branch is pushing too, under its key: a push of it alone waits); their remote statuses are updated from the
 * result. The branch whose remote moved when nothing was pushed (`wholeStack`: its
 * stack branch), with what its remote has now; null when they were pushed; undefined when the push failed (it said why).
 */
async function sendTogether(
  request: VcsRequest,
  branches: readonly Branch[],
  tips: ReadonlyMap<string, string>,
  stack: StackBranch | null,
  report: PushReport,
): Promise<{ name: string; wholeStack: boolean; status: RemoteStatus } | null | undefined> {
  const names = branches.map(branch => branch.name)
  // what an error names: the branches, then the stack branch
  const subjects = stack === null ? names : [...names, formatWholeStackBranch(stack.root)]
  const pushing = stack === null ? names : [...names, stackReadinessKey(stack.root)]
  // the repository the push started in: its result applies there, also after a switch of the repository
  const repositoryPath = request.repositoryPath
  for (const name of pushing) {
    setBranchPushing(name, true, repositoryPath)
  }
  try {
    const progress = new Channel<TransferProgress>()
    progress.onmessage = pushProgress => {
      setBranchesPushProgress(pushing, pushProgress, repositoryPath)
    }
    const result = await commands.pushBranches(
      {
        repositoryPath: request.repositoryPath,
        branchPrefix: request.branchPrefix,
        myEmail: branches[0]?.myEmail ?? null,
        baselineBranch: branchSyncStore.getState().baselineBranch!,
        branches: [
          ...branches.map(branch => ({
            branchName: branch.name,
            // a stacked branch's own commits are those after its parent
            parentBranch: branch.stack?.effectiveBase ?? null,
            // without a loaded status the branch must not be on the remote: if it is, the push fetches and shows it
            expectedRemote: branch.remoteStatus?.remoteTip ?? null,
            tip: tips.get(branch.name) ?? null,
          })),
          ...(stack === null ? [] : [{ branchName: stack.root, wholeStack: true, expectedRemote: stack.remoteStatus?.remoteTip ?? null }]),
        ],
      },
      progress,
    )
    if (result.status === "error") {
      report(`Cannot push ${joinNames(subjects)}`, result.error)
      return undefined
    }
    const statuses = result.data.remoteStatuses.map(status => toRemoteStatus(status))
    branches.forEach((branch, index) => {
      const status = statuses[index]
      if (status !== undefined) {
        setBranchRemoteStatus(branch.name, status, repositoryPath)
      }
    })
    const stackStatus = stack === null ? undefined : statuses[branches.length]
    if (stack !== null && stackStatus !== undefined) {
      setStackBranchRemoteStatus(stack.root, stackStatus, repositoryPath)
    }
    if (result.data.type === "remoteMoved") {
      const { branchName, wholeStack } = result.data
      const index = wholeStack ? branches.length : names.indexOf(branchName)
      const status = statuses[index]
      if (status === undefined) {
        report(`Cannot push ${joinNames(subjects)}`, `The remote branch of ${wholeStack ? formatWholeStackBranch(branchName) : branchName} changed: nothing was pushed`)
        return undefined
      }
      if (wholeStack) {
        recordPush(request.repositoryPath, formatWholeStackBranch(branchName), stack?.remoteStatus ?? null, status)
      } else {
        recordPush(request.repositoryPath, branchName, branches[index]!.remoteStatus, status)
      }
      return { name: branchName, wholeStack, status }
    }
    for (const branch of branches) {
      recordPush(request.repositoryPath, branch.name, sentPart(branch.remoteStatus, tips.get(branch.name)), null)
    }
    if (stack !== null) {
      recordPush(request.repositoryPath, formatWholeStackBranch(stack.root), stack.remoteStatus, null)
    }
    // one refresh of the pull requests, the whole stack's too
    void refreshPullRequestsAfterPushes(branches.map(branch => [branch.name, tips.get(branch.name) ?? branch.commits[0]?.hash] as const))
    return null
  } catch (error) {
    report(`Cannot push ${joinNames(subjects)}`, error)
    return undefined
  } finally {
    for (const name of pushing) {
      setBranchPushing(name, false, repositoryPath)
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
 * A push of the stack branch of a whole stack that runs (`pushWholeStack`, or Push with Children that takes it along):
 * what a second request joins instead of pushing it again
 */
interface StackBranchPush {
  /**
   * Resolves when its push is over: false when the push of the stack branch alone failed (it said why, and the request
   * that joined fails with it), true otherwise; the request that joined then pushes what the remote still lacks
   */
  readonly pushed: Promise<boolean>
  /** The robot it restarts after its push (`restartRobot`): a second restart of that robot is the same request */
  readonly restarts: string | null
  /** The whole request: its push, then its restart */
  readonly done: Promise<boolean>
}

/** The push of the stack branch of each root that runs, the last one asked (it joins the one before) */
const runningStackBranchPushes = new Map<string, StackBranchPush>()

/** Makes `push` the push of the stack branch of `root` a second request joins, until it is done */
function trackStackBranchPush(root: string, push: StackBranchPush): void {
  runningStackBranchPushes.set(root, push)
  const forget = () => {
    if (runningStackBranchPushes.get(root) === push) {
      runningStackBranchPushes.delete(root)
    }
  }
  void push.done.then(forget, forget)
}

/**
 * The push of Push with Children that takes the stack branch of `root` along, for a request of it alone to join: it
 * waits for the push, then pushes what the remote still lacks (nothing once the push took it; the stack branch alone
 * when the push of all of them failed for a branch). The returned function ends it.
 */
function joinableStackBranchPush(root: string): () => void {
  let settle!: (pushed: boolean) => void
  const pushed = new Promise<boolean>(resolve => (settle = resolve))
  trackStackBranchPush(root, { pushed, restarts: null, done: pushed })
  return () => settle(true)
}

/**
 * Pushes the stack branch of the whole stack of `root` (`{prefix}/stack/<root>`), leased on the remote head its status
 * showed, as one more branch of `push_branches` (`wholeStack`): the head of the pull request of the whole stack, the
 * source of its robot. Only a stack branch the sync built (`getStackBranchBlocker`); never once merged Safe Pushes
 * landed every branch of the stack (`getStackLandedReason`: it carries only commits that are in the target), nor while
 * its own Safe Push runs (`getStackPushHold`), unless the push restarts that robot (`restartRobot`, canceled first,
 * restarted on the pushed commit) or a new start replaces it (`replacesRobot`, canceled first). A sync that runs is
 * awaited first. A request while a push of the stack branch runs (another action's, or Push with Children that takes it
 * along) joins it: no push of its own, one indicator; a failed push of it alone is said once, by the request that
 * pushed, and fails the one that joined too; then it pushes only what the remote still lacks (what a sync rebuilt since;
 * the stack branch alone after a Push with Children that failed for a branch), and nothing when the remote has it all.
 * A second restart of the same robot is the same request: one push, one restart, one toast. A remote branch
 * that moved is fetched, and the push goes again over it once when it has no commit the stack branch lacks
 * (`pushesOverMovedRemote`). The branches of the stack stay as they are on the remote. While it runs, the stack branch
 * is pushing under its key (`stackReadinessKey`), its progress where the pushes of the stack show: the island's header,
 * the root's row of a small stack, the button of the start it is a step of. No toast: the action it is a step of says
 * it; Activity records the push. Resolves to whether the stack branch is on the remote.
 */
export function pushWholeStack(root: string, options: WholeStackPushOptions = {}): Promise<boolean> {
  const running = runningStackBranchPushes.get(root)
  const restarts = options.restartRobot?.id ?? null
  if (running !== undefined && restarts !== null && running.restarts === restarts) {
    return running.done
  }
  const pushed = sendWholeStack(root, running?.pushed ?? null, options)
  const done = pushed.then(async isPushed => {
    if (isPushed && options.restartRobot !== undefined) {
      const restarted = await restartAfterPush(options.restartRobot)
      // the restart is the action: its toast is the one toast, Activity has the push and the restart
      if (restarted !== undefined) {
        showRobotToast(`${formatPushMode(restarted.pushMode)} restarted`, restarted)
      }
    }
    return isPushed
  })
  trackStackBranchPush(root, { pushed, restarts, done })
  return done
}

/**
 * The push of `pushWholeStack`: after the push it joins (`joined`), the sync and a push of Push with Children, what the
 * remote still lacks of the stack branch, the robot it restarts or replaces canceled first. Resolves to whether the
 * stack branch is on the remote.
 */
async function sendWholeStack(root: string, joined: Promise<boolean> | null, options: WholeStackPushOptions): Promise<boolean> {
  const report = options.report ?? notifyError
  const subject = formatWholeStack(root)
  const key = stackReadinessKey(root)
  const request = createPushRequest(`Cannot push ${subject}`, report)
  if (request === null) {
    return false
  }
  // a failed push of the stack branch alone was said by the request that pushed: one toast
  if (joined !== null && !(await joined)) {
    return false
  }
  await whenSynced()
  while (isBranchPushing(key)) {
    await whenBranchPushed(key)
    await whenSynced()
  }
  if (!isStillIn(request.repositoryPath, subject, report)) {
    return false
  }
  // its commits are in the target for good: nothing a push sends lands
  const landed = getStackLandedReason(root)
  if (landed !== null) {
    report(`Cannot push ${subject}`, `${landed}: nothing of it is left to push`)
    return false
  }
  // a push that ran meanwhile left nothing to send, unless a sync rebuilt the stack branch since
  const upToDate = getPushAction({ remoteStatus: getStackBranch(root)?.remoteStatus ?? null }) === "upToDate"
  const hold = getStackPushHold(root)
  if (hold !== null) {
    if (hold.robot.id === options.restartRobot?.id || hold.robot.id === options.replacesRobot?.id) {
      if (!(await cancelHoldingRobot(hold.robot, report))) {
        return false
      }
    } else if (!upToDate) {
      report(`Cannot push ${subject}`, formatStackPushHoldReason(root, hold.robot))
      return false
    }
  }
  if (upToDate) {
    return true
  }
  if (!isStillIn(request.repositoryPath, subject, report)) {
    return false
  }
  let moved = await sendStackBranch(request, root, report)
  if (moved !== null && moved !== undefined && pushesOverMovedRemote(moved)) {
    if (!isStillIn(request.repositoryPath, subject, report)) {
      return false
    }
    moved = await sendStackBranch(request, root, report)
  }
  if (moved === undefined) {
    return false
  }
  if (moved !== null) {
    report(`Cannot push ${subject}`, describeMovedStackBranch(moved))
    return false
  }
  return true
}

/**
 * The one push of the stack branch, leased on the remote head of its status, pushing under its key with its progress;
 * its remote status is updated from the result. What its remote has now when it moved, null when it was pushed,
 * undefined when the push failed (it said why).
 */
async function sendStackBranch(request: VcsRequest, root: string, report: PushReport): Promise<RemoteStatus | null | undefined> {
  const subject = formatWholeStack(root)
  const stack = getStackBranch(root)
  const blocker = getStackBranchBlocker(root, stack)
  if (stack === undefined || blocker !== null) {
    report(`Cannot push ${subject}`, blocker ?? "The stack has no stack branch")
    return undefined
  }
  const key = stackReadinessKey(root)
  const repositoryPath = request.repositoryPath
  setBranchPushing(key, true, repositoryPath)
  try {
    const progress = new Channel<TransferProgress>()
    progress.onmessage = pushProgress => {
      setBranchPushProgress(key, pushProgress, repositoryPath)
    }
    const result = await commands.pushBranches(
      {
        repositoryPath: request.repositoryPath,
        branchPrefix: request.branchPrefix,
        myEmail: getBranch(root)?.myEmail ?? null,
        baselineBranch: branchSyncStore.getState().baselineBranch!,
        branches: [{ branchName: root, wholeStack: true, expectedRemote: stack.remoteStatus?.remoteTip ?? null }],
      },
      progress,
    )
    if (result.status === "error") {
      report(`Cannot push ${subject}`, result.error)
      return undefined
    }
    const status = result.data.remoteStatuses[0] === undefined ? null : toRemoteStatus(result.data.remoteStatuses[0])
    if (status !== null) {
      setStackBranchRemoteStatus(root, status, repositoryPath)
    }
    recordPush(request.repositoryPath, formatWholeStackBranch(root), stack.remoteStatus, result.data.type === "remoteMoved" ? status : null)
    if (result.data.type === "remoteMoved") {
      return status ?? undefined
    }
    void refreshPullRequests("push")
    return null
  } catch (error) {
    report(`Cannot push ${subject}`, error)
    return undefined
  } finally {
    setBranchPushing(key, false, repositoryPath)
  }
}
