import { Channel } from "@tauri-apps/api/core"
import type { PatronusRobot, TransferProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorDetails, notifyError } from "@/lib/errors"
import type { EventToastAction } from "@/lib/event-toast"
import { closeEventToasts, showEventToast, toastKeys } from "@/lib/event-toast"
import { retargetBeforePush } from "@/lib/git/new-pull-request"
import type { PushOutcome } from "@/lib/git/push-toast"
import { showPushToast } from "@/lib/git/push-toast"
import { pushStack } from "@/lib/git/stack-push"
import type { VcsRequest } from "@/lib/git/vcs-request"
import { canceledBeforeStart, isLockWaitCanceled } from "@/lib/lock-wait"
import { describeRobot, formatPushMode, openRobot } from "@/lib/patronus"
import { showRobotToast } from "@/lib/patronus-actions"
import { describePush } from "@/lib/push-summary"
import { buildRemoteMovedTexts } from "@/lib/remote-status"
import { buttonStackPushOrder, stackPushOrder } from "@/lib/stack"
import type { ToastActionProps } from "@/lib/toast-message"
import { ACTION_TOAST_TIMEOUT_MS, showToast } from "@/lib/toast-message"
import { pushProgressOf } from "@/lib/transfer-progress"
import { whenSessionViewAt } from "@/model/store"
import { formatPushHoldReason, formatReasonOrNull } from "@/presenters/reasons"
import { recordPush, recordRobot } from "@/stores/activity"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, isBranchPushing, setBranchPushing, setBranchPushProgress, toRemoteStatus, whenBranchPushed, whenSynced } from "@/stores/branch-sync"
import { isRewriteRunning, whenRewritesDone } from "@/stores/operations"
import { cancelPatronusRobot, fetchPatronusRobotStatus, getBranchLanding, getPushHold, restartPatronusRobot, whenRepositoryViewCurrent } from "@/stores/patronus"
import type { PushConfirmTarget } from "@/stores/push-confirm"
import { requestPushConfirm } from "@/stores/push-confirm"
import { getRepositorySwitchReason, getSelectedRepositoryPath, repositoryStore, vcsRequestFactory } from "@/stores/repository"

export interface PushOptions {
  /** The remote commit the push replaces; by default the `remoteTip` of the branch's remote status, the row's view */
  expectedRemote?: string | null
  /**
   * The robot of the branch to restart on the pushed commit (a restart that pushes first, the remote-moved toast); a running one is
   * canceled first: the one push into a branch whose Safe Push runs (`PushHold`)
   */
  restartRobot?: PatronusRobot
  /**
   * The running robot a Safe Push started right after the push replaces (the Safe Push popover): it is canceled, never
   * restarted, so the branch doesn't run twice; the ancestors it lands are pushed too
   */
  replacesRobot?: PatronusRobot
  /**
   * Whether a toast says the push succeeded; false when the push is a step of another action that says how it went (a
   * Safe Push start, a merge request, a restart): one action, one toast. Activity records the push either way, and a
   * failure is an error toast either way.
   */
  announce?: boolean
  /**
   * Push Up to Here: the commit of the branch to push instead of its tip (`PushBranchParams.tip`); the commits after it
   * stay unpushed
   */
  tip?: string
  /**
   * Where a failure is said: an error toast by default (`notifyError`); an action whose step the push is takes the
   * reason to say it in place (`pushForAction`)
   */
  report?: PushReport
}

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

/**
 * Pushes a virtual branch, then refreshes the pull requests (the pull request of the branch now shows the pushed
 * commit); the session of the repository shows the remote status the push left. A branch whose Safe Push runs is not pushed (`checkPushHold`), but by its
 * restart and the start that replaces the robot. Says how it went in a toast (`showPushToast`, unless `announce` is false); errors are reported
 * with a toast. Resolves to whether the push succeeded.
 *
 * The push replaces the remote branch only where the row showed it (`expectedRemote`): a remote that moved since is
 * fetched, not overwritten, and a toast says what it has, with a push over exactly that.
 */
export async function pushBranch(branchName: string, options: PushOptions = {}): Promise<boolean> {
  const { outcome, joined } = await pushOne(branchName, options)
  if (outcome === null) {
    return false
  }
  // a repeat joined the running push: its first caller says how it went
  if (joined) {
    return true
  }
  if (options.announce === false) {
    // the action the push is a step of says how it went; a restart that pushes first says it
    if (outcome.restarted !== undefined) {
      showRobotToast(`${formatPushMode(outcome.restarted.pushMode)} restarted`, outcome.restarted)
    }
  } else {
    showPushToast(branchName, outcome.repositoryPath, describePush(branchName, outcome.before), outcome.restarted, outcome.retargeted)
  }
  return true
}

/** The branch in the table after the sync, or why not; a sync that runs is awaited first, so a missing branch is gone */
function findBranch(branchName: string, report: PushReport): Branch | undefined {
  const branch = getBranch(branchName)
  if (branch === undefined) {
    report(`Cannot push ${branchName}`, `${branchName} is no longer in the branch list`)
  }
  return branch
}

/** What `pushOne` did, in the repository it pushed to: its toast offers that repository's pull request only */
interface OnePush extends PushOutcome {
  readonly repositoryPath: string
}

/** A push of one branch that runs (`pushOne`), by its repository and branch: a repeat of the same push joins it */
interface RunningPush {
  readonly options: PushOptions
  readonly report: PushReport
  promise: Promise<OnePush | null>
  /** What the push said first when it failed, for a joined caller that says it in place */
  failure: Parameters<PushReport> | null
}

const runningPushes = new Map<string, RunningPush>()

/** Whether two pushes of one branch are the same push: the same commit over the same remote, the same robot */
function isSamePush(a: PushOptions, b: PushOptions): boolean {
  return a.tip === b.tip && a.expectedRemote === b.expectedRemote && a.restartRobot?.id === b.restartRobot?.id && a.replacesRobot?.id === b.replacesRobot?.id
}

/**
 * The push of `pushBranch`, without its success toast: what the push did and the robot it restarted (recorded in
 * Activity, the caller says it); null when nothing was pushed (it said why). The push is claimed before its first await:
 * a repeat of the same push (a second click, the push of an action meanwhile) joins it (`joined`) and hears why it
 * failed; another push of the branch (a restart, Push Up to Here) waits for it, then runs.
 */
async function pushOne(branchName: string, options: PushOptions): Promise<{ outcome: OnePush | null; joined: boolean }> {
  const report = options.report ?? notifyError
  const request = createPushRequest(`Cannot push ${branchName}`, report)
  if (request === null) {
    return { outcome: null, joined: false }
  }
  const key = `${request.repositoryPath}\n${branchName}`
  const before = runningPushes.get(key)
  if (before !== undefined && isSamePush(before.options, options)) {
    const outcome = await before.promise
    if (outcome === null && before.failure !== null && before.report !== report) {
      report(...before.failure)
    }
    return { outcome, joined: true }
  }
  const entry: RunningPush = { options, report, promise: Promise.resolve(null), failure: null }
  const own = passingCanceled((title, error, action) => {
    entry.failure ??= [title, error, action]
    report(title, error, action)
  }, report)
  const run = () => pushOneOf(request, branchName, { ...options, report: own }, own)
  // another push of the branch first: this one goes over what it sent
  entry.promise = (before === undefined ? run() : before.promise.catch(() => null).then(run)).finally(() => {
    if (runningPushes.get(key) === entry) {
      runningPushes.delete(key)
    }
  })
  runningPushes.set(key, entry)
  return { outcome: await entry.promise, joined: false }
}

async function pushOneOf(request: VcsRequest, branchName: string, options: PushOptions, report: PushReport): Promise<OnePush | null> {
  const repositoryPath = request.repositoryPath
  // a push pressed while a sync rebuilds the branches or a rewrite of the repository runs (its sync included) waits for
  // them, with the spinner of a push, and pushes the branch as they left it: the row stays as it was during the sync (a
  // quiet resync), so its button stays enabled; a branch the sync brings is pushed too. A stack push that sends the
  // branch is waited for as well: the branch is pushed over what it sent
  if (branchSyncStore.getState().isSyncing || isRewriteRunning(repositoryPath) || isBranchPushing(branchName, repositoryPath)) {
    await whenSettled(branchName, repositoryPath)
  }
  if (!isStillIn(repositoryPath, branchName, report) || findBranch(branchName, report) === undefined) {
    return null
  }

  const clearance = await checkPushHold(branchName, options, report)
  if (clearance === false) {
    return null
  }
  const restartRobot = options.restartRobot

  // the check may have waited for Patronus while a sync changed the branch, or the repository was switched: push it as
  // it is now, only in the repository it started in
  if (!isStillIn(repositoryPath, branchName, report)) {
    return null
  }
  // a stack push that sends the branch started during the check: this push goes over what it sent
  if (isBranchPushing(branchName, repositoryPath)) {
    await whenSettled(branchName, repositoryPath)
    if (!isStillIn(repositoryPath, branchName, report)) {
      return null
    }
  }
  const branch = findBranch(branchName, report)
  if (branch === undefined) {
    return null
  }
  // a pull request into a virtual branch goes into the baseline first, with what it lands
  const retargeted = (await retargetBeforePush(branch)) ?? undefined
  if (!isStillIn(repositoryPath, branchName, report)) {
    return null
  }
  // from here on the push reads nothing of the table: it goes on in its repository with the branch and the baseline as
  // they are now, also after a switch, so the robot it cancels is pushed over and restarted or replaced, never canceled
  // for a push that stops
  const baselineBranch = branchSyncStore.getState().baselineBranch!
  if (clearance.cancels !== null && !(await cancelHoldingRobot(clearance.cancels, report, request))) {
    return null
  }
  // a remote that moved since the row showed it, without commits the branch lacks, the push goes over in Rust, once
  // (`pushedOver`); one that moves again, or one with commits the branch lacks, is told
  const pushed = await pushWithProgress(request, baselineBranch, branch, options, report)
  if (pushed === null) {
    return null
  }
  if (pushed.moved !== null) {
    await showRemoteMovedToast(request, branchName, pushed.moved, restartRobot)
    return null
  }
  // the restart takes the commit just pushed, in the repository of the push
  const restarted = restartRobot === undefined ? undefined : await restartAfterPush(restartRobot, request)
  return { name: branchName, before: sentPart(pushed.before, options.tip), restarted, retargeted, repositoryPath }
}

/**
 * Waits, with the spinner of a push on the row, until no rewrite of the repository runs (its sync included), no sync
 * runs and no other push sends the branch
 */
async function whenSettled(branchName: string, repositoryPath: string): Promise<void> {
  while (isBranchPushing(branchName, repositoryPath)) {
    await whenBranchPushed(branchName, repositoryPath)
  }
  // waiting, not sending: Rust knows of no push yet, so the remote status this sync brings reaches the row, and the
  // push reads it after
  setBranchPushing(branchName, true, repositoryPath)
  try {
    await whenRewritesDone(repositoryPath)
    await whenSynced()
  } finally {
    setBranchPushing(branchName, false, repositoryPath)
  }
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
 * The push itself, while the row shows its progress: the remote status the remote moved to when nothing was pushed
 * (`moved`), else the one the push replaced (`before`: the row's, or the moved remote it went over in Rust); null when it
 * failed (an error toast said so). It resolves once the table has the patch of the session with the push's outcome, so
 * what runs next reads the remote status the push left
 */
async function pushWithProgress(
  request: VcsRequest,
  baselineBranch: string,
  branch: Branch,
  options: PushOptions,
  report: PushReport,
): Promise<{ moved: RemoteStatus | null; before: RemoteStatus | null } | null> {
  const branchName = branch.name
  // the repository the push started in: its result applies there, also after a switch of the repository
  const repositoryPath = request.repositoryPath
  setBranchPushing(branchName, true, repositoryPath)
  try {
    const progress = new Channel<TransferProgress>()
    progress.onmessage = pushProgress => {
      setBranchPushProgress(branchName, pushProgressOf(pushProgress, progress), repositoryPath)
    }
    const result = await commands.pushBranch(
      {
        repositoryPath: request.repositoryPath,
        branchName,
        myEmail: branch.myEmail,
        baselineBranch,
        // a stacked branch's own commits are those after its parent
        parentBranch: branch.stack?.effectiveBase ?? null,
        // without a loaded status the branch must not be on the remote: if it is, the push fetches and shows it
        expectedRemote: options.expectedRemote === undefined ? (branch.remoteStatus?.remoteTip ?? null) : options.expectedRemote,
        tip: options.tip ?? null,
      },
      progress,
    )

    if (result.status === "ok") {
      const { outcome, version } = result.data
      await whenSessionViewAt(repositoryPath, version)
      if (outcome.type === "remoteMoved") {
        const moved = toRemoteStatus(outcome.remoteStatus)
        recordPush(repositoryPath, branchName, branch.remoteStatus, moved)
        return { moved, before: branch.remoteStatus }
      }
      const before = outcome.pushedOver === null ? branch.remoteStatus : toRemoteStatus(outcome.pushedOver)
      recordPush(repositoryPath, branchName, before, null)
      return { moved: null, before }
    }
    reportPushError(report, branchName, result.error)
  } catch (error) {
    report(`Cannot push ${branchName}`, error)
  } finally {
    setBranchPushing(branchName, false, repositoryPath)
  }
  return null
}

/**
 * Restarts a robot the push canceled, on the pushed commit, in the repository of the push (`request`, also after a
 * switch), and records it in Activity there in the words of a restart toast; the caller says it, in the toast of its
 * action. Undefined when the restart failed (an error toast said so).
 */
export async function restartAfterPush(robot: PatronusRobot, request: VcsRequest): Promise<PatronusRobot | undefined> {
  const mode = formatPushMode(robot.pushMode)
  try {
    const restarted = await restartPatronusRobot(robot, request)
    recordRobot(`${mode} restarted`, describeRobot(restarted), restarted, request.repositoryPath)
    return restarted
  } catch (error) {
    notifyError(`Cannot restart the ${mode}`, error)
    return undefined
  }
}

/**
 * Whether a push may go (`checkPushHold`): false when a hold refuses it (it said why); else the robot to cancel right
 * before it sends, the one it restarts or replaces (null for none)
 */
export type PushClearance = false | { readonly cancels: PatronusRobot | null }

const CLEAR: PushClearance = { cancels: null }

/**
 * A branch whose Safe Push runs, or that a Safe Push of a branch built on it lands, is not pushed (`PushHold`): the push
 * won't land, and a push into the robot's own branch makes it fail at the end ("branch was modified"). The rows offer no
 * push then; this refuses one that raced with a start (the IDE, `bd`) or came from a toast. The status is read fresh
 * first (one row on the server), so a robot that finished meanwhile holds nothing. The robot the push restarts or
 * replaces (`restartRobot`, `replacesRobot`) is canceled instead, by the push after its last check, right before it
 * sends (`cancels`), so no refusal follows the cancel. A branch a merged Safe Push landed (`landed`) is refused without a
 * read: its commits are in the target. The holds are those of the repository view of the robots and the branches as
 * they are now (`whenRepositoryViewCurrent`): a robot of a branch built on it holds it only while its commit has
 * commits of it.
 */
export async function checkPushHold(branchName: string, options: PushOptions, report: PushReport): Promise<PushClearance> {
  // the holds of the robots and the branches as they are now: a robot started meanwhile holds it once the view says so
  await whenRepositoryViewCurrent()
  const hold = getPushHold(branchName)
  if (hold === null) {
    return CLEAR
  }
  if (hold.robot.id === options.restartRobot?.id || hold.robot.id === options.replacesRobot?.id) {
    return { cancels: hold.robot }
  }
  const status = hold.kind === "landed" ? null : await fetchPatronusRobotStatus(hold.robot).catch(() => null)
  if (status !== null && status !== "running") {
    return CLEAR
  }
  // the robot that holds it opens from the error: its restart, its cancel
  report(`Cannot push ${branchName}`, formatPushHoldReason(branchName, hold), { children: "Open in Patronus", onClick: () => openRobot(hold.robot) })
  return false
}

/**
 * Cancels the robot of a hold before the push that restarts or replaces it, or before a Safe Push start that replaces it
 * without a push; a robot that finished meanwhile holds nothing, so the push or the start goes on (`replaced`); false
 * when Patronus refused (it said so)
 */
export async function cancelHoldingRobot(robot: PatronusRobot, report: PushReport = notifyError, request: VcsRequest | null = null): Promise<boolean> {
  const mode = formatPushMode(robot.pushMode)
  try {
    await cancelPatronusRobot(robot, request, true)
    return true
  } catch (error) {
    report(`Cannot cancel the ${mode}`, error)
    return false
  }
}

/**
 * Pushes a branch at once, unless the push drops remote commits the branch lacks (someone else's work): then the push
 * button of `target` opens its description with the dropped commits, and the Force Push button there pushes with
 * `options`. The keyboard (P in the preview) and the remote-moved toast push through here, as a click on the button does:
 * a plain push pushes the stack where the button does (`pushFromButton`).
 */
export function pushOrConfirm(branchName: string, target: PushConfirmTarget, options: PushOptions = {}): void {
  const action = getBranch(branchName)?.remoteStatus?.action
  if (action?.kind === "force" && action.drops) {
    requestPushConfirm(branchName, target, options)
    return
  }
  if (options.expectedRemote === undefined && options.restartRobot === undefined) {
    pushFromButton(branchName)
    return
  }
  // the remote-moved toast pushes over exactly what the remote has, the branch alone
  void pushBranch(branchName, options)
}

/**
 * What a click on the push button of the branch pushes: the stack while an ancestor has work for the remote
 * (`buttonStackPushOrder`), else the branch
 */
export function pushFromButton(branchName: string): void {
  if (buttonStackPushOrder(branchName, getBranch, name => getPushHold(name) !== null).length > 0) {
    void pushStack(branchName, { stopAtConfirm: true })
  } else {
    void pushBranch(branchName)
  }
}

/**
 * The remote branch moved since its status was shown, so nothing was pushed: the toast says what the remote has now,
 * and its action pushes over exactly that, through the Safe Push question as any push; a push that drops the commits
 * the remote has now confirms in the row first. A Safe Push the push canceled to restart is restarted by that push;
 * with nothing left to push it is restarted at once, and the toast says so and opens it. The toast is of the repository
 * the push went to (`request`): it offers its push only while the main window shows that one (the branch of the same
 * name in another one is another branch), and closes once the branch is gone; a push of the branch takes its place.
 */
export async function showRemoteMovedToast(request: VcsRequest, branchName: string, remoteStatus: RemoteStatus, restartRobot: PatronusRobot | undefined): Promise<void> {
  const texts = buildRemoteMovedTexts(branchName, remoteStatus)
  // nothing left to push: the remote has the branch's commits, the robot can run on them
  const restarted = restartRobot !== undefined && texts.action === null ? await restartAfterPush(restartRobot, request) : undefined
  let note = ""
  if (restarted !== undefined) {
    note = ` The ${formatPushMode(restarted.pushMode)} restarted on it.`
  } else if (restartRobot !== undefined && texts.action !== null) {
    note = ` The ${formatPushMode(restartRobot.pushMode)} is canceled: the push restarts it.`
  }
  const key = toastKeys.push(request.repositoryPath, branchName)
  const description = `${texts.description}${note}`
  let action: EventToastAction | undefined
  if (texts.action !== null) {
    const label = texts.action
    action = {
      label,
      onClick: () => {
        closeEventToasts(key)
        if (isStillIn(request.repositoryPath, branchName, notifyError)) {
          pushOrConfirm(branchName, "row", { expectedRemote: remoteStatus.remoteTip, restartRobot })
        }
      },
    }
  } else if (restarted !== undefined) {
    action = { label: "Open in Patronus", onClick: () => openRobot(restarted) }
  }
  // the toast of the pushes of the branch: a push of it since takes its place. Its push only while the main window shows
  // the repository, whose branch it pushes (the branch of the same name in another one is another branch), and it closes
  // once the branch is gone
  showEventToast({
    key,
    watch: [branchSyncStore, repositoryStore],
    render: () => {
      if (getRepositorySwitchReason(request.repositoryPath) !== null) {
        return { type: "warning", title: texts.title, description, ...(texts.action === null && { action }), timeout: ACTION_TOAST_TIMEOUT_MS }
      }
      // the user decides: the toast stays until then
      return getBranch(branchName) === undefined
        ? null
        : { type: "warning", title: texts.title, description, action, timeout: texts.action === null && restarted !== undefined ? ACTION_TOAST_TIMEOUT_MS : 0 }
    },
  })
}

/** How the push of an action went: pushed (or nothing to push), else why not, for the action to say in place */
/** A push of an action: pushed, or not with the reason; `canceled` when the user stopped its wait for the lock */
export type ActionPush = { readonly pushed: true } | { readonly pushed: false; readonly reason: string; readonly canceled?: true }

export interface ActionPushOptions {
  /** The running robot a Safe Push start replaces: canceled, never restarted (`PushOptions.replacesRobot`) */
  replacesRobot?: PatronusRobot
  /**
   * The repository the action started in (`VcsRequest.repositoryPath`); by default the selected one at the call. The push
   * refuses once the main window shows another one: it pushes the branches of the table
   */
  repositoryPath?: string
}

/**
 * The push an action needs before it runs (a pull request, a review request, a Safe Push start): the branch and the
 * ancestors with work for the remote, root first, as the push button pushes them, without a toast of its own: the
 * action says how it went. Nothing to push resolves at once. A rewrite of the repository, a sync and a push of the
 * branch already running are awaited, so the action sees the branch as they leave it; a push of the branch or of its
 * stack that runs already is joined, and its failure is the reason. A push the user decides is never made here: one that drops
 * remote commits of the branch or of an ancestor between it and the root refuses with the reason, as does a hold.
 */
export async function pushForAction(branchName: string, options: ActionPushOptions = {}): Promise<ActionPush> {
  const repositoryPath = options.repositoryPath ?? getSelectedRepositoryPath()
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
  // the robot the start replaces lands nothing once the push cancels it: the branches it held are pushed too
  const isHeld = (name: string): boolean => {
    const hold = getPushHold(name)
    return hold !== null && hold.robot.id !== options.replacesRobot?.id
  }
  let reason: string | null = null
  let canceled = false
  const report: PushReport = Object.assign(
    (_title: string, error: unknown) => {
      reason ??= getErrorDetails(error)
    },
    { canceled: () => (canceled = true) },
  )
  const replacesRobot = options.replacesRobot
  const order = stackPushOrder(branchName, getBranch, { stopAtConfirm: true, isHeld })
  let pushed: boolean
  if (order.length === 1 && (branch.remoteStatus?.action.kind ?? "upToDate") === "upToDate") {
    // nothing to push: the robot a start replaces is canceled for the start alone
    pushed = replacesRobot === undefined || (await cancelHoldingRobot(replacesRobot, report))
  } else {
    pushed =
      order.length > 1
        ? await pushStack(branchName, { stopAtConfirm: true, replacesRobot, announce: false, report })
        : await pushBranch(branchName, { replacesRobot, announce: false, report })
  }
  // no reason: the remote moved since the row showed it, and its toast says what the remote has and pushes over it
  if (pushed) {
    return { pushed: true }
  }
  // stopped while it waited for the lock: its toast said so, the action fails nothing
  return canceled
    ? { pushed: false, reason: `Canceled the push of ${branchName} before it started`, canceled: true }
    : { pushed: false, reason: reason ?? `The remote branch of ${branchName} changed: nothing was pushed` }
}
