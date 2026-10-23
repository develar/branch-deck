import { Channel } from "@tauri-apps/api/core"
import { toast } from "@/components/ui/toast"
import type { PatronusRobot, TransferProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorDetails, notifyError } from "@/lib/errors"
import { retargetBeforePush } from "@/lib/git/new-pull-request"
import type { PushOutcome } from "@/lib/git/push-toast"
import { showPushToast } from "@/lib/git/push-toast"
import { pushStack } from "@/lib/git/stack-push"
import type { VcsRequest } from "@/lib/git/vcs-request"
import { describeRobot, formatPushHoldReason, formatPushMode, openRobot } from "@/lib/patronus"
import { showRobotToast } from "@/lib/patronus-actions"
import { describePush, dropsRemoteCommits, getPushAction } from "@/lib/push-summary"
import { buildRemoteMovedTexts } from "@/lib/remote-status"
import { buttonStackPushOrder, needsPush, stackAncestors, stackPushOrder } from "@/lib/stack"
import type { ToastActionProps } from "@/lib/toast-message"
import { ACTION_TOAST_TIMEOUT_MS } from "@/lib/toast-message"
import { recordPush, recordRobot } from "@/stores/activity"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import {
  branchSyncStore,
  getBranch,
  isBranchPushing,
  setBranchPushing,
  setBranchPushProgress,
  setBranchRemoteStatus,
  toRemoteStatus,
  whenBranchPushed,
  whenSynced,
} from "@/stores/branch-sync"
import { cancelPatronusRobot, fetchPatronusRobotStatus, getPushHold, restartPatronusRobot, whenRepositoryViewCurrent } from "@/stores/patronus"
import { refreshPullRequestsAfterPush } from "@/stores/pull-requests"
import type { PushConfirmTarget } from "@/stores/push-confirm"
import { requestPushConfirm } from "@/stores/push-confirm"
import { getRepositorySwitchReason, getSelectedRepositoryPath, vcsRequestFactory } from "@/stores/repository"

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

/** Says why a push failed or was refused: the title of an error toast and its reason */
export type PushReport = (title: string, error: unknown, action?: ToastActionProps) => void

/**
 * Pushes a virtual branch and updates its remote status from the result, then the pull requests (the pull request of
 * the branch now shows the pushed commit). A branch whose Safe Push runs is not pushed (`checkPushHold`), but by its
 * restart and the start that replaces the robot. Says how it went in a toast (`showPushToast`, unless `announce` is false); errors are reported
 * with a toast. Resolves to whether the push succeeded.
 *
 * The push replaces the remote branch only where the row showed it (`expectedRemote`): a remote that moved since is
 * fetched, not overwritten, and a toast says what it has, with a push over exactly that.
 */
export async function pushBranch(branchName: string, options: PushOptions = {}): Promise<boolean> {
  const outcome = await pushOne(branchName, options)
  if (outcome === null) {
    return false
  }
  if (options.announce === false) {
    // the action the push is a step of says how it went; a restart that pushes first says it
    if (outcome.restarted !== undefined) {
      showRobotToast(`${formatPushMode(outcome.restarted.pushMode)} restarted`, outcome.restarted)
    }
  } else {
    showPushToast(branchName, describePush(branchName, outcome.before), outcome.restarted, outcome.retargeted)
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

/**
 * The push of `pushBranch`, without its success toast: what the push did and the robot it restarted (recorded in
 * Activity, the caller says it); null when nothing was pushed (it said why)
 */
async function pushOne(branchName: string, options: PushOptions): Promise<PushOutcome | null> {
  if (isBranchPushing(branchName)) {
    return null
  }
  const report = options.report ?? notifyError
  const request = createPushRequest(`Cannot push ${branchName}`, report)
  if (request === null) {
    return null
  }
  const repositoryPath = request.repositoryPath
  // a push pressed while a sync rebuilds the branches waits for it, with the spinner of a push, and pushes the branch as
  // the sync left it: the row stays as it was during the sync (a quiet resync), so its button stays enabled; a branch the
  // sync brings is pushed too
  if (branchSyncStore.getState().isSyncing) {
    setBranchPushing(branchName, true, repositoryPath)
    await whenSynced()
    setBranchPushing(branchName, false, repositoryPath)
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
  const branch = findBranch(branchName, report)
  if (branch === undefined || isBranchPushing(branchName)) {
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
  let before = branch.remoteStatus
  let pushed = await pushWithProgress(request, baselineBranch, branch, options, report)
  // the remote moved since the row showed it, without commits the branch lacks: nobody's work is lost, so the push goes
  // over what the remote has now, once (a remote that moves again, or one with commits the branch lacks, is told)
  const moved = pushed?.moved ?? null
  if (moved !== null && pushesOverMovedRemote(moved)) {
    before = moved
    pushed = await pushWithProgress(request, baselineBranch, { ...branch, remoteStatus: moved }, { ...options, expectedRemote: moved.remoteTip }, report)
  }
  if (pushed === null) {
    return null
  }
  if (pushed.moved !== null) {
    await showRemoteMovedToast(request, branchName, pushed.moved, restartRobot)
    return null
  }
  // the restart takes the commit just pushed, in the repository of the push
  const restarted = restartRobot === undefined ? undefined : await restartAfterPush(restartRobot, request)
  return { name: branchName, before: sentPart(before, options.tip), restarted, retargeted }
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

/**
 * The remote moved since the row showed it, but has no commits the branch lacks, and the branch has work for it: nobody's
 * work is lost, so the push goes over what the remote has now, once (the push of a branch and of a stack)
 */
export function pushesOverMovedRemote(moved: RemoteStatus): boolean {
  return !dropsRemoteCommits(moved) && getPushAction({ remoteStatus: moved }) !== "upToDate"
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
 * The push itself, while the row shows its progress: the remote status the remote moved to when nothing was pushed
 * (`moved`), null when it failed (an error toast said so)
 */
async function pushWithProgress(
  request: VcsRequest,
  baselineBranch: string,
  branch: Branch,
  options: PushOptions,
  report: PushReport,
): Promise<{ moved: RemoteStatus | null } | null> {
  const branchName = branch.name
  // the repository the push started in: its result applies there, also after a switch of the repository
  const repositoryPath = request.repositoryPath
  setBranchPushing(branchName, true, repositoryPath)
  try {
    const progress = new Channel<TransferProgress>()
    progress.onmessage = pushProgress => {
      setBranchPushProgress(branchName, pushProgress, repositoryPath)
    }
    const result = await commands.pushBranch(
      {
        repositoryPath: request.repositoryPath,
        branchPrefix: request.branchPrefix,
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
      const remoteStatus = toRemoteStatus(result.data.remoteStatus)
      setBranchRemoteStatus(branchName, remoteStatus, repositoryPath)
      recordPush(repositoryPath, branchName, branch.remoteStatus, result.data.type === "remoteMoved" ? remoteStatus : null)
      if (result.data.type === "remoteMoved") {
        return { moved: remoteStatus }
      }
      void refreshPullRequestsAfterPush(branchName, options.tip ?? branch.commits[0]?.hash)
      return { moved: null }
    }
    report(`Cannot push ${branchName}`, result.error)
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
 * without a push; false when Patronus refused (it said so)
 */
export async function cancelHoldingRobot(robot: PatronusRobot, report: PushReport = notifyError, request: VcsRequest | null = null): Promise<boolean> {
  const mode = formatPushMode(robot.pushMode)
  try {
    await cancelPatronusRobot(robot, request)
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
  const remoteStatus = getBranch(branchName)?.remoteStatus ?? null
  if (remoteStatus !== null && dropsRemoteCommits(remoteStatus)) {
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
 * the push went to (`request`): its push refuses while the main window shows another one, whose branch of the same name
 * is another branch.
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
  const id = toast.add({
    type: "warning",
    title: texts.title,
    description: `${texts.description}${note}`,
    // the user decides: the toast stays until then
    timeout: texts.action === null && restarted !== undefined ? ACTION_TOAST_TIMEOUT_MS : 0,
    ...(texts.action !== null && {
      actionProps: {
        children: texts.action,
        onClick: () => {
          toast.close(id)
          if (isStillIn(request.repositoryPath, branchName, notifyError)) {
            pushOrConfirm(branchName, "row", { expectedRemote: remoteStatus.remoteTip, restartRobot })
          }
        },
      },
    }),
    ...(texts.action === null &&
      restarted !== undefined && {
        actionProps: { children: "Open in Patronus", onClick: () => openRobot(restarted) },
      }),
  })
}

/** How the push of an action went: pushed (or nothing to push), else why not, for the action to say in place */
export type ActionPush = { readonly pushed: true } | { readonly pushed: false; readonly reason: string }

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
 * action says how it went. Nothing to push resolves at once. A sync and a push of the branch already running are
 * awaited, so the action sees the branch as they leave it. A push the user decides is never made here: one that drops
 * remote commits of the branch or of an ancestor between it and the root refuses with the reason, as does a hold.
 */
export async function pushForAction(branchName: string, options: ActionPushOptions = {}): Promise<ActionPush> {
  const repositoryPath = options.repositoryPath ?? getSelectedRepositoryPath()
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
  const isHeld = (name: string) => isHeldBy(name, options.replacesRobot)
  const blocker = actionPushBlocker(branch, getBranch, isHeld)
  if (blocker !== null) {
    return { pushed: false, reason: blocker }
  }
  let reason: string | null = null
  const report: PushReport = (_title, error) => {
    reason ??= getErrorDetails(error)
  }
  const replacesRobot = options.replacesRobot
  const order = stackPushOrder(branchName, getBranch, { stopAtConfirm: true, isHeld })
  let pushed: boolean
  if (order.length === 1 && !needsPush(branch)) {
    // nothing to push: the robot a start replaces is canceled for the start alone
    pushed = replacesRobot === undefined || (await cancelHoldingRobot(replacesRobot, report))
  } else {
    pushed =
      order.length > 1
        ? await pushStack(branchName, { stopAtConfirm: true, replacesRobot, announce: false, report })
        : await pushBranch(branchName, { replacesRobot, announce: false, report })
  }
  // no reason: the remote moved since the row showed it, and its toast says what the remote has and pushes over it
  return pushed ? { pushed: true } : { pushed: false, reason: reason ?? `The remote branch of ${branchName} changed: nothing was pushed` }
}

/**
 * Why the push an action needs can't be made for the user: the last sync did not build the branch or an ancestor it is
 * built on (its virtual branch is an older build, which the backend refuses to push), or the push drops commits of its
 * remote branch (someone else's work; its row asks first); null when it can
 */
export function actionPushBlocker(branch: Branch, branchOf: (name: string) => Branch | undefined, isHeld: (name: string) => boolean): string | null {
  for (const it of [branch, ...stackAncestors(branch.name, branchOf).reverse()]) {
    // the robot of a held ancestor lands it: the branches above it are pushed, as the push button pushes them
    if (it !== branch && isHeld(it.name)) {
      break
    }
    // not "nothing to push": the action would go on with a remote branch of commits the branch may no longer have
    if (it.hasError || it.status === "Blocked") {
      return `${it.name} can't be pushed: ${it.statusText === "" ? "it failed to sync" : it.statusText}`
    }
    if (it.remoteStatus != null && needsPush(it) && dropsRemoteCommits(it.remoteStatus)) {
      const count = it.remoteStatus.commitsBehind
      return `The remote branch of ${it.name} has ${count === 1 ? "a commit" : `${count} commits`} that ${it.name} lacks: push it from its row, which shows ${count === 1 ? "it" : "them"}`
    }
  }
  return null
}

/** A Safe Push holds the branch (`getPushHold`), other than the robot a start replaces */
export function isHeldBy(branchName: string, replaces: PatronusRobot | undefined): boolean {
  const hold = getPushHold(branchName)
  return hold !== null && hold.robot.id !== replaces?.id
}
