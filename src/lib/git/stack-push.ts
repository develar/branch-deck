import { Channel } from "@tauri-apps/api/core"
import type { PatronusRobot, TransferProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { Retargeted } from "@/lib/git/new-pull-request"
import { retargetBeforePush } from "@/lib/git/new-pull-request"
import type { PushClearance, PushReport } from "@/lib/git/push"
import {
  cancelHoldingRobot,
  checkPushHold,
  createPushRequest,
  isHeldBy,
  isStillIn,
  passingCanceled,
  pushesOverMovedRemote,
  reportPushError,
  restartAfterPush,
  sentPart,
  showRemoteMovedToast,
} from "@/lib/git/push"
import type { PushOutcome } from "@/lib/git/push-toast"
import { showStackPushToast } from "@/lib/git/push-toast"
import type { VcsRequest } from "@/lib/git/vcs-request"
import { formatPushMode, formatStackPushHoldReason } from "@/lib/patronus"
import { showRobotToast } from "@/lib/patronus-actions"
import { getPushAction } from "@/lib/push-summary"
import type { PushUpToStep } from "@/lib/push-up-to"
import { stackChildrenPushOrder, stackPushOrder } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import { pushProgressOf } from "@/lib/transfer-progress"
import { formatWholeStack, formatWholeStackBranch, getStackBranchAlong, getStackBranchBlocker, stackReadinessKey } from "@/lib/whole-stack"
import { whenSessionViewAt } from "@/model/store"
import { recordPush } from "@/stores/activity"
import type { Branch, RemoteStatus, StackBranch } from "@/stores/branch-sync"
import {
  branchSyncStore,
  getBranch,
  getStackBranch,
  getUnparkedStackDescendants,
  isBranchPushing,
  setBranchesPushProgress,
  setBranchPushing,
  setBranchPushProgress,
  setStackPush,
  toRemoteStatus,
  whenBranchPushed,
  whenSynced,
} from "@/stores/branch-sync"
import { isRewriteRunning, whenRewritesDone } from "@/stores/operations"
import { getPushHold, getStackBranchHoldReason, getStackLandedReason, getStackPushHold } from "@/stores/patronus"
import { getStackPullRequest, refreshPullRequests, refreshPullRequestsAfterPushes } from "@/stores/pull-requests"
import { isSelectedRepository } from "@/stores/repository"

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
    `stack ${options.stopAtConfirm === true} ${options.replacesRobot?.id ?? ""}`,
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
    "children",
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
  return await pushTogether(last.name, `up to ${JSON.stringify([...tips])}`, () => ({ names: steps.map(step => step.name), tips }), {})
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
 * the same row while one runs (`intent`: the same order) joins it and hears why it failed; another one from the row
 * waits for it, then runs. The branches of the order are claimed once it is known, before the checks: a push of one of
 * them meanwhile waits for this one instead of sending it twice.
 * Resolves to whether the branches were pushed; true when nothing is left to push.
 */
async function pushTogether(startedFrom: string, intent: string, orderOf: () => StackPushOrder, options: StackPushOptions): Promise<boolean> {
  const report = options.report ?? notifyError
  const request = createPushRequest(`Cannot push ${startedFrom}`, report)
  if (request === null) {
    return false
  }
  const key = `${request.repositoryPath}\n${startedFrom}`
  const before = runningStackPushes.get(key)
  if (before !== undefined && before.intent === intent) {
    const pushed = await before.promise
    if (!pushed && before.failure !== null && before.report !== report) {
      report(...before.failure)
    }
    return pushed
  }
  const entry: RunningStackPush = { intent, report, promise: Promise.resolve(false), failure: null }
  const own = passingCanceled((title, error, action) => {
    entry.failure ??= [title, error, action]
    report(title, error, action)
  }, report)
  const run = () => pushTogetherOf(request, startedFrom, orderOf, { ...options, report: own })
  // another push from the row first: this one goes over what it sent; else the row shows the push at once
  entry.promise = (before === undefined ? run() : before.promise.catch(() => false).then(run)).finally(() => {
    if (runningStackPushes.get(key) === entry) {
      runningStackPushes.delete(key)
    }
  })
  runningStackPushes.set(key, entry)
  return await entry.promise
}

/** A stack push that runs (`pushTogether`), by its repository and the row it started from */
interface RunningStackPush {
  readonly intent: string
  readonly report: PushReport
  promise: Promise<boolean>
  /** What the push said first when it failed, for a joined caller that says it in place */
  failure: Parameters<PushReport> | null
}

const runningStackPushes = new Map<string, RunningStackPush>()

async function pushTogetherOf(request: VcsRequest, startedFrom: string, orderOf: () => StackPushOrder, options: StackPushOptions): Promise<boolean> {
  const report = options.report ?? notifyError
  const repositoryPath = request.repositoryPath
  // the branches of the table are of the selected repository: the push goes on only while it is the one it started in
  const stillIn = () => isStillIn(repositoryPath, startedFrom, report)
  // the row shows the push while it waits for a sync, and a second click does nothing
  setStackPush(startedFrom, [], repositoryPath)
  // a push of the stack branch alone asked meanwhile (Request Review…, a restart) waits for this one, which may take it
  let settleStackBranch: (() => void) | null = null
  // the branches of the order, claimed until the push sends them: a push of one of them meanwhile waits for this one
  let claimed: readonly string[] = []
  const release = () => {
    for (const name of claimed) {
      setBranchPushing(name, false, repositoryPath)
    }
    claimed = []
  }
  try {
    const order = await settledOrder(orderOf, repositoryPath)
    claimed = order.names
    for (const name of claimed) {
      setBranchPushing(name, true, repositoryPath)
    }
    if (!stillIn()) {
      return false
    }
    const wholeStack = order.wholeStack ?? null
    if (wholeStack !== null) {
      settleStackBranch = joinableStackBranchPush(wholeStack)
    }
    if (order.names.length === 0 && wholeStack === null) {
      return true
    }
    const clearance = await checkStackHolds(order.names, options, report)
    if (clearance === false) {
      return false
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
    // a pull request into a virtual branch goes into the baseline first, with what it lands; each retarget reads the
    // pull request of the selected repository, so none starts after a switch during the one before
    const retargeted = new Map<string, Retargeted>()
    for (const branch of branches) {
      if (!stillIn()) {
        return false
      }
      const it = await retargetBeforePush(branch)
      if (it !== null) {
        retargeted.set(branch.name, it)
      }
    }
    if (!stillIn()) {
      return false
    }
    // from here on the push reads nothing of the table: it goes on in its repository with the branches and the baseline
    // as they are now, also after a switch, so the robot it cancels is pushed over and replaced, never canceled for a push
    // that stops
    const baselineBranch = branchSyncStore.getState().baselineBranch!
    let stack = stackBranchAlong(wholeStack)
    if (clearance.cancels !== null && !(await cancelHoldingRobot(clearance.cancels, report, request))) {
      return false
    }
    setStackPush(startedFrom, order.names, repositoryPath)
    // the send marks them pushing itself, at once
    release()
    let pushed: readonly Branch[] = branches
    let moved = await sendTogether(request, baselineBranch, pushed, tips, stack, report)
    // nobody's work is lost: the stack goes over what the remotes have now, once, as the push of one branch does; a
    // remote that moves again, or one with commits its branch lacks, is told
    if (moved != null && pushesOverMovedRemote(moved.status)) {
      pushed = moved.branches
      stack = moved.stack
      moved = await sendTogether(request, baselineBranch, pushed, tips, stack, report)
    }
    if (moved === undefined) {
      return false
    }
    if (moved !== null) {
      if (moved.wholeStack) {
        report(`Cannot push ${formatWholeStackBranch(moved.name)}`, describeMovedStackBranch(moved.status))
      } else {
        await showRemoteMovedToast(request, moved.name, moved.status, undefined)
      }
      return false
    }
    if (options.announce !== false) {
      const outcomes: PushOutcome[] = pushed.map(branch => ({
        name: branch.name,
        before: sentPart(branch.remoteStatus, tips.get(branch.name)),
        retargeted: retargeted.get(branch.name),
      }))
      // after a switch the pull requests of the table are of the other repository: the toast offers none
      const pullRequest = stack === null || !isSelectedRepository(repositoryPath) ? undefined : getStackPullRequest(stack.root)
      showStackPushToast(outcomes, startedFrom, repositoryPath, stack === null ? null : { root: stack.root, before: stack.remoteStatus, pullRequest })
    }
    return true
  } finally {
    release()
    settleStackBranch?.()
    setStackPush(startedFrom, null, repositoryPath)
  }
}

/** A stack push the remote refused (`sendTogether`): the branch whose remote moved, and what the push sends over them */
interface MovedInStack {
  readonly name: string
  readonly wholeStack: boolean
  /** What its remote has now */
  readonly status: RemoteStatus
  /** The branches of the push with the remote statuses it brought */
  readonly branches: readonly Branch[]
  /** The stack branch it took along, with the remote status it brought */
  readonly stack: StackBranch | null
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
 * The order once no rewrite of the repository runs (its sync included), no sync runs and no push of one of its
 * branches from another row, nor of the stack branch it takes along (a request of it alone, its restart included): a
 * stack push waits for them and goes on the branches as they leave them, the stack branch along only while its remote
 * still lacks it
 */
async function settledOrder(orderOf: () => StackPushOrder, repositoryPath: string): Promise<StackPushOrder> {
  while (true) {
    await whenRewritesDone(repositoryPath)
    await whenSynced()
    if (isRewriteRunning(repositoryPath)) {
      continue
    }
    const order = orderOf()
    const names = order.wholeStack == null ? order.names : [...order.names, stackReadinessKey(order.wholeStack)]
    const running = names.filter(name => isBranchPushing(name, repositoryPath))
    const stackPush = order.wholeStack == null ? undefined : runningStackBranchPushes.get(order.wholeStack)
    if (running.length === 0 && stackPush === undefined) {
      return order
    }
    await Promise.all([...running.map(name => whenBranchPushed(name, repositoryPath)), stackPush?.done])
  }
}

/**
 * Whether no running Safe Push holds a branch of the push (`checkPushHold`), so the push may go: one held branch
 * refuses the whole push, with the reason. The robot a Safe Push start replaces is canceled once, by the push right
 * before it sends (`cancels`), whichever of its branches it holds.
 */
async function checkStackHolds(names: readonly string[], options: StackPushOptions, report: PushReport): Promise<PushClearance> {
  let cancels: PatronusRobot | null = null
  for (const name of names) {
    const hold = getPushHold(name)
    if (hold !== null && hold.robot.id === cancels?.id) {
      continue
    }
    const clearance = await checkPushHold(name, { replacesRobot: options.replacesRobot }, report)
    if (clearance === false) {
      return false
    }
    cancels ??= clearance.cancels
  }
  return { cancels }
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
 * stack branch is pushing too, under its key: a push of it alone waits); it resolves once the table has the patch of the
 * session with the remote statuses it left. The branch whose remote moved when nothing was pushed (`wholeStack`: its
 * stack branch), with what its remote has now, and the branches and the stack branch with the remote statuses the push
 * brought, for a push over them; null when they were pushed; undefined when the push failed (it said why).
 */
async function sendTogether(
  request: VcsRequest,
  baselineBranch: string,
  branches: readonly Branch[],
  tips: ReadonlyMap<string, string>,
  stack: StackBranch | null,
  report: PushReport,
): Promise<MovedInStack | null | undefined> {
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
      setBranchesPushProgress(pushing, pushProgressOf(pushProgress, progress), repositoryPath)
    }
    const result = await commands.pushBranches(
      {
        repositoryPath: request.repositoryPath,
        myEmail: branches[0]?.myEmail ?? null,
        baselineBranch,
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
      reportPushError(report, joinNames(subjects), result.error)
      return undefined
    }
    const { outcome, version } = result.data
    const statuses = outcome.remoteStatuses.map(status => toRemoteStatus(status))
    await whenSessionViewAt(repositoryPath, version)
    const stackStatus = stack === null ? undefined : statuses[branches.length]
    if (outcome.type === "remoteMoved") {
      const { branchName, wholeStack } = outcome
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
      return {
        name: branchName,
        wholeStack,
        status,
        branches: branches.map((branch, at) => ({ ...branch, remoteStatus: statuses[at] ?? branch.remoteStatus })),
        stack: stack === null ? null : { ...stack, remoteStatus: stackStatus ?? stack.remoteStatus },
      }
    }
    for (const branch of branches) {
      recordPush(request.repositoryPath, branch.name, sentPart(branch.remoteStatus, tips.get(branch.name)), null)
    }
    if (stack !== null) {
      recordPush(request.repositoryPath, formatWholeStackBranch(stack.root), stack.remoteStatus, null)
    }
    // one refresh of the pull requests, the whole stack's too
    void refreshPullRequestsAfterPushes(
      branches.map(branch => [branch.name, tips.get(branch.name) ?? branch.commits[0]?.hash] as const),
      request,
    )
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
  const request = createPushRequest(`Cannot push ${formatWholeStack(root)}`, options.report ?? notifyError)
  if (request === null) {
    return Promise.resolve(false)
  }
  const pushed = sendWholeStack(request, root, running?.pushed ?? null, options)
  const done = pushed.then(async isPushed => {
    if (isPushed && options.restartRobot !== undefined) {
      // in the repository of the push, also after a switch
      const restarted = await restartAfterPush(options.restartRobot, request)
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
 * The push of `pushWholeStack` in the repository of `request`: after the push it joins (`joined`), the sync and a push
 * of Push with Children, what the remote still lacks of the stack branch, the robot it restarts or replaces canceled
 * after the last check, right before it sends, so no refusal follows the cancel. Resolves to whether the stack branch is
 * on the remote.
 */
async function sendWholeStack(request: VcsRequest, root: string, joined: Promise<boolean> | null, options: WholeStackPushOptions): Promise<boolean> {
  const report = options.report ?? notifyError
  const subject = formatWholeStack(root)
  const key = stackReadinessKey(root)
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
  let cancels: PatronusRobot | null = null
  if (hold !== null) {
    if (hold.robot.id === options.restartRobot?.id || hold.robot.id === options.replacesRobot?.id) {
      cancels = hold.robot
    } else if (!upToDate) {
      report(`Cannot push ${subject}`, formatStackPushHoldReason(root, hold.robot))
      return false
    }
  }
  // from here on the push reads nothing of the table: it goes on in its repository with the stack branch as it is now,
  // also after a switch, so the robot it cancels is pushed over and restarted or replaced, never canceled for nothing
  const sent = upToDate ? null : stackBranchToSend(root, report)
  if (sent === undefined) {
    return false
  }
  if (cancels !== null && !(await cancelHoldingRobot(cancels, report, request))) {
    return false
  }
  if (sent === null) {
    return true
  }
  let moved = await sendStackBranch(request, sent, report)
  if (moved !== null && moved !== undefined && pushesOverMovedRemote(moved)) {
    moved = await sendStackBranch(request, { ...sent, stack: { ...sent.stack, remoteStatus: moved } }, report)
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

/** What the push of a stack branch sends, as the table had it when the push started (`stackBranchToSend`) */
interface StackBranchToSend {
  readonly root: string
  readonly stack: StackBranch
  readonly myEmail: string | null
  readonly baselineBranch: string
}

/** The stack branch of `root` and what its push needs, from the table; undefined when it can't be pushed (it said why) */
function stackBranchToSend(root: string, report: PushReport): StackBranchToSend | undefined {
  const stack = getStackBranch(root)
  const blocker = getStackBranchBlocker(root, stack)
  if (stack === undefined || blocker !== null) {
    report(`Cannot push ${formatWholeStack(root)}`, blocker ?? "The stack has no stack branch")
    return undefined
  }
  return { root, stack, myEmail: getBranch(root)?.myEmail ?? null, baselineBranch: branchSyncStore.getState().baselineBranch! }
}

/**
 * The one push of the stack branch, leased on the remote head of its status, pushing under its key with its progress;
 * it resolves once the table has the patch of the session with the remote status it left. What its remote has now when
 * it moved, null when it was pushed, undefined when the push failed (it said why).
 */
async function sendStackBranch(request: VcsRequest, sent: StackBranchToSend, report: PushReport): Promise<RemoteStatus | null | undefined> {
  const { root, stack } = sent
  const subject = formatWholeStack(root)
  const key = stackReadinessKey(root)
  const repositoryPath = request.repositoryPath
  setBranchPushing(key, true, repositoryPath)
  try {
    const progress = new Channel<TransferProgress>()
    progress.onmessage = pushProgress => {
      setBranchPushProgress(key, pushProgressOf(pushProgress, progress), repositoryPath)
    }
    const result = await commands.pushBranches(
      {
        repositoryPath: request.repositoryPath,
        myEmail: sent.myEmail,
        baselineBranch: sent.baselineBranch,
        branches: [{ branchName: root, wholeStack: true, expectedRemote: stack.remoteStatus?.remoteTip ?? null }],
      },
      progress,
    )
    if (result.status === "error") {
      reportPushError(report, subject, result.error)
      return undefined
    }
    const { outcome, version } = result.data
    const status = outcome.remoteStatuses[0] === undefined ? null : toRemoteStatus(outcome.remoteStatuses[0])
    await whenSessionViewAt(repositoryPath, version)
    recordPush(request.repositoryPath, formatWholeStackBranch(root), stack.remoteStatus, outcome.type === "remoteMoved" ? status : null)
    if (outcome.type === "remoteMoved") {
      return status ?? undefined
    }
    void refreshPullRequests("push", request)
    return null
  } catch (error) {
    report(`Cannot push ${subject}`, error)
    return undefined
  } finally {
    setBranchPushing(key, false, repositoryPath)
  }
}
