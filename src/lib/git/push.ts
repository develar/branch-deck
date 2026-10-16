import { Channel } from "@tauri-apps/api/core"
import { toast } from "@/components/ui/toast"
import type { PatronusRobot, TransferProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { retargetBeforePush } from "@/lib/git/new-pull-request"
import type { PushOutcome } from "@/lib/git/push-toast"
import { showPushToast, showStackPushToast } from "@/lib/git/push-toast"
import type { PushHold } from "@/lib/patronus"
import { describeRobot, formatPushHoldReason, formatPushMode, openRobot } from "@/lib/patronus"
import { showRobotToast } from "@/lib/patronus-actions"
import { describePush, dropsRemoteCommits } from "@/lib/push-summary"
import type { PushUpToStep } from "@/lib/push-up-to"
import { buildRemoteMovedTexts } from "@/lib/remote-status"
import { buttonStackPushOrder, stackChildrenPushOrder, stackPushOrder } from "@/lib/stack"
import { ACTION_TOAST_TIMEOUT_MS } from "@/lib/toast-message"
import { recordPush, recordRobot } from "@/stores/activity"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, getStackDescendants, isBranchPushing, isStackPushing, setBranchPushing, setBranchPushProgress, setBranchRemoteStatus, setStackPushCurrent, toRemoteStatus, whenSynced } from "@/stores/branch-sync"
import { cancelPatronusRobot, fetchPatronusRobotStatus, getPushHold, restartPatronusRobot } from "@/stores/patronus"
import { refreshPullRequestsAfterPush } from "@/stores/pull-requests"
import type { PushConfirmTarget } from "@/stores/push-confirm"
import { requestPushConfirm } from "@/stores/push-confirm"
import { vcsRequestFactory } from "@/stores/repository"

export interface PushOptions {
  /** The remote commit the push replaces; by default the `remoteTip` of the branch's remote status, the row's view */
  expectedRemote?: string | null
  /**
   * The robot of the branch to restart on the pushed commit (Push and Restart, the remote-moved toast); a running one is
   * canceled first: the one push into a branch whose Safe Push runs (`PushHold`)
   */
  restartRobot?: PatronusRobot
  /**
   * The running robot a Safe Push started right after the push replaces (the Safe Push form): it is canceled, never
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
}

/**
 * Pushes a virtual branch and updates its remote status from the result, then the pull requests (the pull request of
 * the branch now shows the pushed commit). A branch whose Safe Push runs is not pushed (`checkPushHold`), but by Push
 * and Restart and the start that replaces the robot. Says how it went in a toast (`showPushToast`, unless `announce` is false); errors are reported
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
    // the action the push is a step of says how it went; a restart is the action of "Push and Restart"
    if (outcome.restarted !== undefined) {
      showRobotToast(`${formatPushMode(outcome.restarted.pushMode)} restarted`, outcome.restarted)
    }
  }
  else {
    showPushToast(branchName, describePush(branchName, outcome.before), outcome.restarted, outcome.retargeted)
  }
  return true
}

/** The branch in the table, or an error toast */
function findBranch(branchName: string): Branch | undefined {
  const branch = getBranch(branchName)
  if (branch === undefined) {
    notifyError(`Cannot push ${branchName}`, "The branch is not in the table: sync and try again.")
  }
  return branch
}

/**
 * The push of `pushBranch`, without its success toast: what the push did and the robot it restarted (recorded in
 * Activity, the caller says it); null when nothing was pushed (it said why)
 */
async function pushOne(branchName: string, options: PushOptions): Promise<PushOutcome | null> {
  if (isBranchPushing(branchName) || findBranch(branchName) === undefined) {
    return null
  }
  // a push pressed while a sync rebuilds the branches waits for it, with the spinner of a push, and pushes the branch as
  // the sync left it: the row stays as it was during the sync (a quiet resync), so its button stays enabled
  if (branchSyncStore.getState().isSyncing) {
    setBranchPushing(branchName, true)
    await whenSynced()
    setBranchPushing(branchName, false)
  }

  if (!await checkPushHold(branchName, options)) {
    return null
  }
  const restartRobot = options.restartRobot

  // the check may have waited for Patronus while a sync changed the branch: push it as it is now
  const branch = findBranch(branchName)
  if (branch === undefined || isBranchPushing(branchName)) {
    return null
  }
  // pushed onto another base, its pull request into the old one would show the wrong commits
  const retargeted = await retargetBeforePush(branch) ?? undefined
  const pushed = await pushWithProgress(branch, options)
  if (pushed === null) {
    return null
  }
  if (pushed.moved !== null) {
    await showRemoteMovedToast(branchName, pushed.moved, restartRobot)
    return null
  }
  // the restart takes the commit just pushed
  const restarted = restartRobot === undefined ? undefined : await restartAfterPush(restartRobot)
  return { name: branchName, before: sentPart(branch.remoteStatus, options.tip), restarted, retargeted }
}

/** The remote status a push up to `tip` was made against, as its toast counts it: the commits up to it are ahead */
function sentPart(before: RemoteStatus | null, tip: string | undefined): RemoteStatus | null {
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
async function pushWithProgress(branch: Branch, options: PushOptions): Promise<{ moved: RemoteStatus | null } | null> {
  const branchName = branch.name
  setBranchPushing(branchName, true)
  try {
    const request = vcsRequestFactory.createRequest()
    const progress = new Channel<TransferProgress>()
    progress.onmessage = (pushProgress) => {
      setBranchPushProgress(branchName, pushProgress)
    }
    const result = await commands.pushBranch({
      repositoryPath: request.repositoryPath,
      branchPrefix: request.branchPrefix,
      branchName,
      myEmail: branch.myEmail,
      baselineBranch: branchSyncStore.getState().baselineBranch!,
      // a stacked branch's own commits are those after its parent
      parentBranch: branch.stack?.effectiveBase ?? null,
      // without a loaded status the branch must not be on the remote: if it is, the push fetches and shows it
      expectedRemote: options.expectedRemote === undefined ? branch.remoteStatus?.remoteTip ?? null : options.expectedRemote,
      tip: options.tip ?? null,
    }, progress)

    if (result.status === "ok") {
      const remoteStatus = toRemoteStatus(result.data.remoteStatus)
      setBranchRemoteStatus(branchName, remoteStatus)
      recordPush(request.repositoryPath, branchName, branch.remoteStatus, result.data.type === "remoteMoved" ? remoteStatus : null)
      if (result.data.type === "remoteMoved") {
        return { moved: remoteStatus }
      }
      void refreshPullRequestsAfterPush(branchName, options.tip ?? branch.commits[0]?.hash)
      return { moved: null }
    }
    notifyError(`Cannot push ${branchName}`, result.error)
  }
  catch (error) {
    notifyError(`Cannot push ${branchName}`, error)
  }
  finally {
    setBranchPushing(branchName, false)
  }
  return null
}

/**
 * Restarts a robot the push canceled, on the pushed commit, and records it in Activity in the words of a restart
 * toast; the caller says it, in the toast of its action. Undefined when the restart failed (an error toast said so).
 */
async function restartAfterPush(robot: PatronusRobot): Promise<PatronusRobot | undefined> {
  const mode = formatPushMode(robot.pushMode)
  try {
    const restarted = await restartPatronusRobot(robot)
    recordRobot(`${mode} restarted`, describeRobot(restarted), restarted)
    return restarted
  }
  catch (error) {
    notifyError(`Cannot restart the ${mode}`, error)
    return undefined
  }
}

/**
 * A branch whose Safe Push runs, or that a Safe Push of a branch built on it lands, is not pushed (`PushHold`): the push
 * won't land, and a push into the robot's own branch makes it fail at the end ("branch was modified"). The rows offer no
 * push then; this refuses one that raced with a start (the IDE, `bd`) or came from a toast. The status is read fresh
 * first (one row on the server), so a robot that finished meanwhile holds nothing. The robot the push restarts or
 * replaces (`restartRobot`, `replacesRobot`) is canceled instead, and the push goes on. Resolves to whether to push.
 */
async function checkPushHold(branchName: string, options: PushOptions): Promise<boolean> {
  const hold = getPushHold(branchName)
  if (hold === null) {
    return true
  }
  if (hold.robot.id === options.restartRobot?.id || hold.robot.id === options.replacesRobot?.id) {
    return await cancelHoldingRobot(hold)
  }
  const status = await fetchPatronusRobotStatus(hold.robot).catch(() => null)
  if (status !== null && status !== "running") {
    return true
  }
  notifyError(`Cannot push ${branchName}`, formatPushHoldReason(branchName, hold))
  return false
}

/** Cancels the robot of a hold before the push that restarts or replaces it; false when Patronus refused (it said so) */
async function cancelHoldingRobot(hold: PushHold): Promise<boolean> {
  const mode = formatPushMode(hold.robot.pushMode)
  try {
    await cancelPatronusRobot(hold.robot)
    return true
  }
  catch (error) {
    notifyError(`Cannot cancel the ${mode}`, error)
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
  }
  else {
    void pushBranch(branchName)
  }
}

/**
 * The remote branch moved since its status was shown, so nothing was pushed: the toast says what the remote has now,
 * and its action pushes over exactly that, through the Safe Push question as any push; a push that drops the commits
 * the remote has now confirms in the row first. A Safe Push the push canceled to restart is restarted by that push;
 * with nothing left to push it is restarted at once, and the toast says so and opens it.
 */
async function showRemoteMovedToast(branchName: string, remoteStatus: RemoteStatus, restartRobot: PatronusRobot | undefined): Promise<void> {
  const texts = buildRemoteMovedTexts(branchName, remoteStatus)
  // nothing left to push: the remote has the branch's commits, the robot can run on them
  const restarted = restartRobot !== undefined && texts.action === null ? await restartAfterPush(restartRobot) : undefined
  let note = ""
  if (restarted !== undefined) {
    note = ` The ${formatPushMode(restarted.pushMode)} restarted on it.`
  }
  else if (restartRobot !== undefined && texts.action !== null) {
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
          pushOrConfirm(branchName, "row", { expectedRemote: remoteStatus.remoteTip, restartRobot })
        },
      },
    }),
    ...(texts.action === null && restarted !== undefined && {
      actionProps: { children: "Open in Patronus", onClick: () => openRobot(restarted) },
    }),
  })
}

export interface StackPushOptions {
  /** The order of the push button: it starts above an ancestor whose push drops remote commits (`stackPushOrder`) */
  stopAtConfirm?: boolean
  /** The push is a step of a Safe Push start: the robot it replaces is canceled, never restarted (`PushOptions`) */
  replacesRobot?: PatronusRobot
  /** Whether a toast says what was pushed (`PushOptions`) */
  announce?: boolean
}

/**
 * Pushes the ancestors of a stacked branch that have work for the remote, root first, then the branch; stops at a
 * failure. Resolves to whether every branch was pushed.
 */
export async function pushStack(branchName: string, options: StackPushOptions = {}): Promise<boolean> {
  return await pushInOrder(stackPushOrder(branchName, getBranch, { stopAtConfirm: options.stopAtConfirm, isHeld: name => isHeldBy(name, options.replacesRobot) }), branchName, options)
}

/** A Safe Push holds the branch (`getPushHold`), other than the robot a start replaces */
function isHeldBy(branchName: string, replaces: PatronusRobot | undefined): boolean {
  const hold = getPushHold(branchName)
  return hold !== null && hold.robot.id !== replaces?.id
}

/** Pushes a branch and the branches stacked on it that have work for the remote, parents first; stops at a failure */
export async function pushWithChildren(branchName: string): Promise<void> {
  await pushInOrder(stackChildrenPushOrder(branchName, getStackDescendants(branchName), getBranch, name => isHeldBy(name, undefined)), branchName, {})
}

/**
 * Push Up to Here (`pushUpToPlan`): the branches the commit is built on that have work for the remote, root first, then
 * its branch up to it; stops at a failure, one toast for all
 */
export async function pushUpTo(steps: readonly PushUpToStep[]): Promise<boolean> {
  const last = steps.at(-1)
  if (last === undefined) {
    return false
  }
  const tips = new Map(steps.flatMap(step => step.tip === undefined ? [] : [[step.name, step.tip] as const]))
  return await pushInOrder(steps.map(step => step.name), last.name, {}, tips)
}

/**
 * Pushes the branches in order while the row of `startedFrom` shows which one it pushes, then one toast says what was
 * pushed, also before a failure (`showStackPushToast`). A second push from the same row while one runs does nothing.
 * `tips`: the branches pushed up to a commit (Push Up to Here).
 */
async function pushInOrder(names: readonly string[], startedFrom: string, options: StackPushOptions, tips: ReadonlyMap<string, string> = new Map()): Promise<boolean> {
  if (isStackPushing(startedFrom)) {
    return false
  }
  const outcomes: PushOutcome[] = []
  try {
    for (const name of names) {
      setStackPushCurrent(startedFrom, name)
      // a Safe Push start cancels the robot it replaces, at the first branch that robot holds: the branch or an ancestor
      const outcome = await pushOne(name, { replacesRobot: options.replacesRobot, tip: tips.get(name) })
      if (outcome === null) {
        break
      }
      outcomes.push(outcome)
    }
  }
  finally {
    setStackPushCurrent(startedFrom, null)
  }
  if (options.announce !== false) {
    showStackPushToast(outcomes, startedFrom)
  }
  else {
    for (const outcome of outcomes) {
      if (outcome.restarted !== undefined) {
        showRobotToast(`${formatPushMode(outcome.restarted.pushMode)} restarted`, outcome.restarted)
      }
    }
  }
  return outcomes.length === names.length
}
