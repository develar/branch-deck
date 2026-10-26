import { Channel } from "@tauri-apps/api/core"
import type { PatronusRobot, TransferProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorDetails, notifyError } from "@/lib/errors"
import type { PushOutcome } from "@/lib/git/push-toast"
import { showStackPushToast } from "@/lib/git/push-toast"
import type { VcsRequest } from "@/lib/git/vcs-request"
import { canceledBeforeStart, isLockWaitCanceled } from "@/lib/lock-wait"
import { describeRobot, formatPushMode, openRobot } from "@/lib/patronus"
import type { PushRun } from "@/lib/push-run"
import { pushBranch, runBranchPush, showRemoteMovedToast } from "@/lib/push-run"
import type { PushUpToStep } from "@/lib/push-up-to"
import { showRobotToast } from "@/lib/robot-toast"
import { runFailure, runIntent } from "@/lib/run-intent"
import { joinNames } from "@/lib/text"
import type { ToastActionProps } from "@/lib/toast-message"
import { showToast } from "@/lib/toast-message"
import { pushProgressOf } from "@/lib/transfer-progress"
import { formatWholeStack, formatWholeStackBranch, stackReadinessKey } from "@/lib/whole-stack"
import { whenSessionViewAt } from "@/model/store"
import type { Retargeted } from "@/presenters/pull-request-url"
import { getPullRequestRetarget } from "@/presenters/pull-request-url"
import { formatPushHoldReason, formatReason, formatReasonOrNull, formatStackPushHoldReason } from "@/presenters/reasons"
import { recordPush, recordRobot } from "@/stores/activity"
import type { Branch, PushOrders, RemoteStatus, StackBranch } from "@/stores/branch-sync"
import {
  branchSyncStore,
  getBranch,
  getPushOrders,
  getStackBranch,
  getStackNotBuilt,
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
import { getBranchLanding, getPushHold, getStackLandedReason, getStackPushHold, whenRepositoryViewCurrent } from "@/stores/patronus"
import { getStackPullRequest, retargetPullRequest } from "@/stores/pull-requests"
import type { PushConfirmTarget } from "@/stores/push-confirm"
import { getRepositorySwitchReason, getSelectedRepositoryPath, isSelectedRepository, vcsRequestFactory } from "@/stores/repository"

// The pushes of the page that are no intents of the engine yet (s9-13): Push Stack, Push with Children, Push Up to Here,
// the push of the stack branch of a whole stack, the button that pushes the stack while an ancestor has work, and the
// push an action needs before it runs. The push of a branch alone is the intent `Push` (`push-run.ts`).

/**
 * Says why a push failed or was refused: the title of an error toast and its reason. `canceled` hears a push whose wait
 * for another holder of the repository lock the user stopped (Stop Waiting): nothing was pushed and nothing failed, its
 * info toast said so, so an action whose step the push is ends without an error of its own (`pushForAction`)
 */
export type PushReport = ((title: string, error: unknown, action?: ToastActionProps) => void) & { readonly canceled?: () => void }

/** `own`, a report that passes on to `report`, with the `canceled` of `report` */
export function passingCanceled(own: (title: string, error: unknown, action?: ToastActionProps) => void, report: PushReport): PushReport {
  const canceled = report.canceled
  return canceled === undefined ? own : Object.assign(own, { canceled })
}

/** The repository and branch prefix a push starts in, or null when none is configured (`report` said why) */
export function createPushRequest(title: string, report: PushReport): VcsRequest | null {
  try {
    return vcsRequestFactory.createRequest()
  } catch (error) {
    report(title, error)
    return null
  }
}

/**
 * Whether the main window still shows the repository the push of `subject` started in: a push never goes on with the
 * branches of another repository, whose branch of the same name is another branch (`report` says it stopped)
 */
export function isStillIn(repositoryPath: string, subject: string, report: PushReport): boolean {
  const reason = getRepositorySwitchReason(repositoryPath)
  if (reason !== null) {
    report(`Cannot push ${subject}`, reason)
  }
  return reason === null
}

/** The remote status a push up to `tip` was made against, as its toast counts it: the commits up to it are ahead */
export function sentPart(before: RemoteStatus | null, tip: string | undefined): RemoteStatus | null {
  if (before === null || tip === undefined) {
    return before
  }
  const sent = before.unpushedCommits.indexOf(tip) + 1
  return sent === 0 ? before : { ...before, commitsAhead: sent, myCommitsAhead: Math.min(before.myCommitsAhead, sent) }
}

/**
 * A failed push of `subject` as `report` says it ("Cannot push auth"); a push whose wait for another holder of the
 * repository lock the user stopped (Stop Waiting) pushed nothing and failed nothing: an info toast says so
 */
export function reportPushError(report: PushReport, subject: string, error: unknown): void {
  if (isLockWaitCanceled(error)) {
    showToast(canceledBeforeStart(`the push of ${subject}`, "Nothing was pushed"))
    report.canceled?.()
  } else {
    report(`Cannot push ${subject}`, error)
  }
}

/**
 * Restarts a robot the push canceled, on the pushed commit, in the repository of the push (`request`, also after a
 * switch): the restart of the engine (`RestartRobot`), whose plan pushes nothing more now; Activity records it there in
 * the words of a restart toast, the caller says it in the toast of its action. Undefined when the restart failed or its
 * plan refused it (an error toast said so)
 */
export async function restartAfterPush(robot: PatronusRobot, request: VcsRequest): Promise<PatronusRobot | undefined> {
  const mode = formatPushMode(robot.pushMode)
  const run = await runIntent(request.repositoryPath, { kind: "restartRobot", robotId: robot.id })
  const restarted = run.kind === "ran" && run.outcome.result.kind === "robot" ? run.outcome.result.robot : null
  if (restarted === null) {
    notifyError(`Cannot restart the ${mode}`, runFailure(run, `Patronus offers no restart of it now`))
    return undefined
  }
  recordRobot(`${mode} restarted`, describeRobot(restarted), restarted, request.repositoryPath)
  return restarted
}

/** The robot a push restarts or replaces: canceled right before it sends, so no hold refuses it */
interface HoldOptions {
  readonly restartRobot?: PatronusRobot
  readonly replacesRobot?: PatronusRobot
}

/**
 * Whether a push may go (`checkPushHold`): false when a hold refuses it (it said why); else the robot to cancel right
 * before it sends, the one it restarts or replaces (null for none), and the running robot the view says holds it
 * (`holder`, null for none), which Rust refuses the push for while it runs and its error opens
 */
export type PushClearance = false | { readonly cancels: PatronusRobot | null; readonly holder: PatronusRobot | null }

const CLEAR: PushClearance = { cancels: null, holder: null }

/**
 * A branch whose Safe Push runs, or that a Safe Push of a branch built on it lands, is not pushed (`PushHold`): the push
 * won't land, and a push into the robot's own branch makes it fail at the end ("branch was modified"). The rows offer no
 * push then; a push that raced with a start (the IDE, `bd`) or came from a toast goes to Rust, which reads the user's
 * robots now and refuses it while the robot runs (one that finished meanwhile holds nothing); its error opens the robot
 * (`holder`). The robot the push restarts or replaces (`restartRobot`, `replacesRobot`) is canceled instead, by the
 * push after its last check, right before it sends (`cancels`), so no refusal follows the cancel. A branch a merged Safe
 * Push landed (`landed`) is refused here: its commits are in the target. The holds are those of the repository view of
 * the robots and the branches as they are now (`whenRepositoryViewCurrent`): a robot of a branch built on it holds it
 * only while its commit has commits of it.
 */
export async function checkPushHold(branchName: string, options: HoldOptions, report: PushReport): Promise<PushClearance> {
  // the holds of the robots and the branches as they are now: a robot started meanwhile holds it once the view says so
  await whenRepositoryViewCurrent()
  const hold = getPushHold(branchName)
  if (hold === null) {
    return CLEAR
  }
  if (hold.robot.id === options.restartRobot?.id || hold.robot.id === options.replacesRobot?.id) {
    return { cancels: hold.robot, holder: null }
  }
  if (hold.kind !== "landed") {
    return { cancels: null, holder: hold.robot }
  }
  // the robot that landed it opens from the error
  report(`Cannot push ${branchName}`, formatPushHoldReason(branchName, hold), { children: "Open in Patronus", onClick: () => openRobot(hold.robot) })
  return false
}

/**
 * Cancels the robot of a hold before the push that restarts or replaces it, or before a Safe Push start that replaces it
 * without a push: the cancel of the engine (`CancelRobot`); a robot that finished meanwhile holds nothing, so the push or
 * the start goes on; false when Patronus refused (it said so)
 */
export async function cancelHoldingRobot(robot: PatronusRobot, report: PushReport = notifyError, request: VcsRequest | null = null): Promise<boolean> {
  const mode = formatPushMode(robot.pushMode)
  const run = await runIntent(request?.repositoryPath ?? getSelectedRepositoryPath(), { kind: "cancelRobot", robotId: robot.id })
  const failure = runFailure(run, `Patronus lists no robot ${robot.id} of the user`)
  if (failure !== null) {
    report(`Cannot cancel the ${mode}`, failure)
    return false
  }
  return true
}

/**
 * Retargets the open pull request of a branch whose base is a virtual branch to the baseline before the branch is
 * pushed (`getPullRequestRetarget`), as the engine runs it (`retargetPullRequest`, Activity records it): the pull request
 * then shows what the Safe Push of the branch lands, and its approvals count. Null when there was nothing to retarget,
 * or the retarget failed (an error toast said why; the push still runs, its toast offers the command).
 */
export async function retargetBeforePush(branch: Branch): Promise<Retargeted | null> {
  const retarget = getPullRequestRetarget(branch.name)
  if (retarget === null) {
    return null
  }
  const { pullRequest, base } = retarget
  try {
    await retargetPullRequest(branch.name)
  } catch (error) {
    notifyError(`Cannot retarget pull request #${pullRequest.number}`, error)
    return null
  }
  return { number: pullRequest.number, base }
}

/**
 * What a click on the push button of the branch pushes (`confirmAt` the button, which asks first when the push drops
 * remote commits): the stack while an ancestor has work for the remote, as Rust orders it (`PushOrders.button`), else
 * the branch alone (`pushBranch`)
 */
export function pushFromButton(branchName: string, confirmAt: PushConfirmTarget = "row"): void {
  if (getPushOrders(branchName).button.length > 0) {
    void pushStack(branchName, { stopAtConfirm: true })
  } else {
    void pushBranch(branchName, { confirmAt })
  }
}

/**
 * The push of a button or a key away from it (the branch menu, P in the preview): what a click on the push button of
 * `confirmAt` pushes (`pushFromButton`), whose description asks first for a push that drops remote commits
 */
export function pushOrConfirm(branchName: string, confirmAt: PushConfirmTarget): void {
  pushFromButton(branchName, confirmAt)
}

/** A push of an action: pushed, or not with the reason; `canceled` when the user stopped its wait for the lock */
export type ActionPush = { readonly pushed: true } | { readonly pushed: false; readonly reason: string; readonly canceled?: true }

export interface ActionPushOptions {
  /** The running robot a Safe Push start replaces: canceled, never restarted (`PushOptions.replacesRobot`) */
  replacesRobot?: PatronusRobot
  /**
   * The branches a Safe Push start pushes, root first, as Rust decided them (the `push` effect of its decision): the
   * start pushes those and no other; by default the push of an action (`PushOrders.action`)
   */
  branches?: readonly string[]
  /**
   * The repository the action started in (`VcsRequest.repositoryPath`); by default the selected one at the call. The push
   * refuses once the main window shows another one: it pushes the branches of the table
   */
  repositoryPath?: string
}

/**
 * The push an action needs before it runs ("Push" of the toast of a start that can't run): the branch and the
 * ancestors with work for the remote, root first, as the push button pushes them, without a toast of its own: the
 * action says how it went. Nothing to push resolves at once. A rewrite of the repository, a sync and a push of the
 * branch already running are awaited, so the action sees the branch as they leave it. A push the user decides is never
 * made here: one that drops remote commits of the branch or of an ancestor between it and the root refuses with the
 * reason, as does a hold.
 */
export async function pushForAction(branchName: string): Promise<ActionPush> {
  const repositoryPath = getSelectedRepositoryPath()
  // a rewrite of the repository that runs (a move out of the branch) and its sync first: the push sends what it leaves
  await whenRewritesDone(repositoryPath)
  await whenSynced()
  await whenBranchPushed(branchName)
  // the holds decide where the push starts: a parent a running robot's commit doesn't have is pushed too
  await whenRepositoryViewCurrent()
  // the branches of the table are of the selected repository: another one's branch of the same name is another branch
  const switched = getRepositorySwitchReason(repositoryPath)
  if (switched !== null) {
    return { pushed: false, reason: switched }
  }
  const branch = getBranch(branchName)
  if (branch === undefined) {
    return { pushed: false, reason: `${branchName} is no longer in the branch list` }
  }
  // why the push can't be made for the user, as Rust decides it of the view (`domain_policy::holds::push_blocker`): an
  // unbuilt branch of its chain, a push that drops remote commits
  const blocker = formatReasonOrNull(getBranchLanding(branchName).push, name => getBranch(name)?.statusText ?? "")
  if (blocker !== null) {
    return { pushed: false, reason: blocker }
  }
  const order = getPushOrders(branchName).action
  if (order.length === 1 && (branch.remoteStatus?.action.kind ?? "upToDate") === "upToDate") {
    return { pushed: true }
  }
  if (order.length === 1 && order[0] === branchName) {
    return actionPushOf(await runBranchPush(branchName, { announce: false }), branchName)
  }
  let reason: string | null = null
  let canceled = false
  const report: PushReport = Object.assign(
    (_title: string, error: unknown) => {
      reason ??= getErrorDetails(error)
    },
    { canceled: () => (canceled = true) },
  )
  if (await pushStack(branchName, { stopAtConfirm: true, announce: false, report })) {
    return { pushed: true }
  }
  // stopped while it waited for the lock: its toast said so, the action fails nothing
  return canceled
    ? { pushed: false, reason: `Canceled the push of ${branchName} before it started`, canceled: true }
    : { pushed: false, reason: reason ?? `The remote branch of ${branchName} changed: nothing was pushed` }
}

/** How the push of the branch alone went, as the action it is a step of says it */
function actionPushOf(run: PushRun, branchName: string): ActionPush {
  switch (run.kind) {
    case "pushed":
      return { pushed: true }
    case "failed":
      return { pushed: false, reason: getErrorDetails(run.error) }
    case "canceled":
      return { pushed: false, reason: `Canceled the push of ${branchName} before it started`, canceled: true }
    case "confirm":
      return { pushed: false, reason: `The push of ${branchName} drops remote commits it lacks: its row asks first` }
    case "moved":
      return { pushed: false, reason: `The remote branch of ${branchName} changed: nothing was pushed` }
  }
}

export interface StackPushOptions {
  /** The order of the push button: it starts above an ancestor whose push drops remote commits (`PushOrders.action`) */
  stopAtConfirm?: boolean
  /**
   * The branches to push, root first, as Rust decided them for a Safe Push start (the `push` effect of its decision,
   * `domain_policy::push::start_push`); by default the order of the button or of Push Stack (`PushOrders`)
   */
  order?: readonly string[]
  /** The push is a step of a Safe Push start: the robot it replaces is canceled, never restarted (`PushOptions`) */
  replacesRobot?: PatronusRobot
  /** Whether a toast says what was pushed (`PushOptions`) */
  announce?: boolean
  /** Where a failure is said (`PushOptions`) */
  report?: PushReport
}

/**
 * What a stack push sends: the branches, parents first, the ones pushed up to a commit (Push Up to Here), and the root
 * whose stack branch of the whole stack goes along after them (Push with Children of the root, `ChildrenPushOrder.stackBranch`)
 */
interface StackPushOrder {
  names: readonly string[]
  tips?: ReadonlyMap<string, string>
  wholeStack?: string | null
}

/**
 * Push Stack: the ancestors of a stacked branch that have work for the remote, root first, then the branch, in one
 * atomic push (`pushTogether`), as Rust orders them (`PushOrders`): from the push button, above the nearest one that asks
 * first (`action`); of a Safe Push start what its decision pushes (`order`). Resolves to whether they were pushed.
 */
export async function pushStack(branchName: string, options: StackPushOptions = {}): Promise<boolean> {
  const orderOf = (orders: PushOrders): readonly string[] => options.order ?? (options.stopAtConfirm === true ? orders.action : orders.stack)
  return await pushTogether(
    branchName,
    `stack ${options.stopAtConfirm === true} ${options.replacesRobot?.id ?? ""} ${options.order?.join(",") ?? ""}`,
    () => ({ names: orderOf(getPushOrders(branchName)) }),
    options,
  )
}

/**
 * Push with Children: the branch if it has work for the remote, then the branches stacked on it that have work, parents
 * first, and for the root of a stack that forks the stack branch of its whole stack while it goes along, as Rust orders
 * them (`ChildrenPush`), in one atomic push (`pushTogether`), so the pull request of the whole stack follows its branches
 */
export async function pushWithChildren(branchName: string): Promise<boolean> {
  return await pushTogether(
    branchName,
    "children",
    () => ({
      names: getPushOrders(branchName).children?.order ?? [],
      wholeStack: stackBranchGoesAlong(branchName) ? branchName : null,
    }),
    {},
  )
}

/**
 * Rust's answer whether Push with Children of `root` takes the stack branch of its whole stack along now
 * (`stack_branch_along`): never while it is held (`StackLanding.hold`): the robot of the whole stack runs, a push into
 * its stack branch would fail it, or every branch of the stack landed, a push of it lands nothing
 */
function stackBranchGoesAlong(root: string): boolean {
  return getPushOrders(root).children?.stackBranch?.kind === "along"
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
 * the whole push: the branches are fetched, and Rust pushes again over what the remotes have now, once, when that
 * remote has no commits its branch lacks (`pushedOver`), else its toast says what it has. A second push from
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
    const stack = stackBranchAlong(wholeStack)
    if (clearance.cancels !== null && !(await cancelHoldingRobot(clearance.cancels, report, request))) {
      return false
    }
    setStackPush(startedFrom, order.names, repositoryPath)
    // the send marks them pushing itself, at once
    release()
    // nobody's work is lost: Rust pushes the stack again over what the remotes have now, once, as the push of one branch;
    // a remote that moves again, or one with commits its branch lacks, is told
    const sent = await sendTogether(request, baselineBranch, branches, tips, stack, report)
    if (sent === undefined) {
      return false
    }
    if ("moved" in sent) {
      const moved = sent.moved
      if (moved.wholeStack) {
        report(`Cannot push ${formatWholeStackBranch(moved.name)}`, describeMovedStackBranch(moved.status))
      } else {
        await showRemoteMovedToast(request.repositoryPath, moved.name, moved.status, false)
      }
      return false
    }
    if (options.announce !== false) {
      const outcomes: PushOutcome[] = sent.branches.map(branch => ({
        name: branch.name,
        before: sentPart(branch.remoteStatus, tips.get(branch.name)),
        retargeted: retargeted.get(branch.name),
      }))
      // after a switch the pull requests of the table are of the other repository: the toast offers none
      const pushedStack = sent.stack
      const pullRequest = pushedStack === null || !isSelectedRepository(repositoryPath) ? undefined : getStackPullRequest(pushedStack.root)
      showStackPushToast(outcomes, startedFrom, repositoryPath, pushedStack === null ? null : { root: pushedStack.root, before: pushedStack.remoteStatus, pullRequest })
    }
    return true
  } finally {
    release()
    settleStackBranch?.()
    setStackPush(startedFrom, null, repositoryPath)
  }
}

/** A stack push the remote refused (`sendTogether`): the branch whose remote moved, and what its remote has now */
interface MovedInStack {
  readonly name: string
  readonly wholeStack: boolean
  readonly status: RemoteStatus
}

/**
 * A stack push that went (`sendTogether`): its branches and the stack branch it took along, each with the remote status
 * the push replaced (the row's, or the moved remote Rust pushed over)
 */
interface PushedStack {
  readonly branches: readonly Branch[]
  readonly stack: StackBranch | null
}

/** The stack branch of the whole stack of `root` as the push sends it: only while it still goes along; null otherwise */
function stackBranchAlong(root: string | null): StackBranch | null {
  return root !== null && stackBranchGoesAlong(root) ? (getStackBranch(root) ?? null) : null
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
 * Whether no Safe Push holds a branch of the push (`checkPushHold`), so the push may go: one landed branch refuses the
 * whole push, with the reason; a running robot of one Rust refuses it for (`holder`). The robot a Safe Push start
 * replaces is canceled once, by the push right before it sends (`cancels`), whichever of its branches it holds.
 */
async function checkStackHolds(names: readonly string[], options: StackPushOptions, report: PushReport): Promise<PushClearance> {
  let cancels: PatronusRobot | null = null
  let holder: PatronusRobot | null = null
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
    holder ??= clearance.holder
  }
  return { cancels, holder }
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
 * session with the remote statuses it left. The branch whose remote moved when nothing was pushed (`moved`; `wholeStack`:
 * its stack branch), with what its remote has now; else the branches and the stack branch with the remote statuses the
 * push replaced (`pushedOver` when Rust pushed again over a moved remote); undefined when the push failed (it said why).
 */
async function sendTogether(
  request: VcsRequest,
  baselineBranch: string,
  branches: readonly Branch[],
  tips: ReadonlyMap<string, string>,
  stack: StackBranch | null,
  report: PushReport,
): Promise<{ moved: MovedInStack } | PushedStack | undefined> {
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
      return { moved: { name: branchName, wholeStack, status } }
    }
    // the statuses the push replaced: those Rust fetched when it pushed again over a moved remote, else the rows'
    const over = outcome.pushedOver.map(status => toRemoteStatus(status))
    const pushed = branches.map((branch, at) => ({ ...branch, remoteStatus: over[at] ?? branch.remoteStatus }))
    const pushedStack = stack === null ? null : { ...stack, remoteStatus: over[branches.length] ?? stack.remoteStatus }
    for (const branch of pushed) {
      recordPush(request.repositoryPath, branch.name, sentPart(branch.remoteStatus, tips.get(branch.name)), null)
    }
    if (pushedStack !== null) {
      recordPush(request.repositoryPath, formatWholeStackBranch(pushedStack.root), pushedStack.remoteStatus, null)
    }
    // the session lists the pull requests after the push, the whole stack's too
    return { branches: pushed, stack: pushedStack }
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
 * source of its robot. Only a stack branch the sync built (`getStackNotBuilt`); never once merged Safe Pushes
 * landed every branch of the stack (`getStackLandedReason`: it carries only commits that are in the target), nor while
 * its own Safe Push runs (`getStackPushHold`), unless the push restarts that robot (`restartRobot`, canceled first,
 * restarted on the pushed commit) or a new start replaces it (`replacesRobot`, canceled first). A sync that runs is
 * awaited first. A request while a push of the stack branch runs (another action's, or Push with Children that takes it
 * along) joins it: no push of its own, one indicator; a failed push of it alone is said once, by the request that
 * pushed, and fails the one that joined too; then it pushes only what the remote still lacks (what a sync rebuilt since;
 * the stack branch alone after a Push with Children that failed for a branch), and nothing when the remote has it all.
 * A second restart of the same robot is the same request: one push, one restart, one toast. A remote branch
 * that moved is fetched, and Rust pushes again over it once when it has no commit the stack branch lacks
 * (`pushedOver`). The branches of the stack stay as they are on the remote. While it runs, the stack branch
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
  const upToDate = getStackBranch(root)?.remoteStatus?.action.kind === "upToDate"
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
  // a remote that moved without commits the stack branch lacks Rust pushes over, once (`pushedOver`)
  const moved = await sendStackBranch(request, sent, report)
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
  const notBuilt = getStackNotBuilt(root)
  if (stack === undefined || notBuilt !== null) {
    report(`Cannot push ${formatWholeStack(root)}`, notBuilt === null ? "The stack has no stack branch" : formatReason(notBuilt))
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
    if (outcome.type === "remoteMoved") {
      recordPush(request.repositoryPath, formatWholeStackBranch(root), stack.remoteStatus, status)
      return status ?? undefined
    }
    const over = outcome.pushedOver[0]
    recordPush(request.repositoryPath, formatWholeStackBranch(root), over === undefined ? stack.remoteStatus : toRemoteStatus(over), null)
    return null
  } catch (error) {
    report(`Cannot push ${subject}`, error)
    return undefined
  } finally {
    setBranchPushing(key, false, repositoryPath)
  }
}
