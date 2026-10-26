// The pushes of the page, from every surface that offers them: the push of a branch alone and of an unapplied branch (the
// push button of the row, of the preview and of the review window, P, the menus, the toast of a remote that moved) and
// the pushes of a stack (the button that pushes the stack, Push Stack, Push with Children, Push Up to Here, the stack
// branch of a whole stack alone), each the intent `Push` Rust plans and runs (`operations::push`;
// spec/repository/outgoing.spec.md, "Push"; spec/branch/stacked-branches.spec.md). Its plan retargets each pull request
// out of a virtual branch first (D3a), cancels and restarts the Safe Push of the branch when the push restarts it, and
// asks first for a push that drops remote commits the branch lacks (`ForcePushDrops`): the push button of the surface
// opens its description with the dropped commits, whose Force Push pushes; a push of a stack whose surface described
// each drop runs with them accepted. The session admits it (the same push twice is one, another push of a ref waits),
// waits for its sync, decides under the repository lock and pushes after it; the view shows it running
// (`pushOperationIn`, `useStackPush`), the window that runs it how far it is. The page words the plan's refusal and the
// outcome: the toast of the pushes, the remote that moved, why not.
import type { ApiError, Intent, Outcome, PatronusRobot, PushScope, Reason } from "@/lib/bindings"
import { getErrorCode, notifyError } from "@/lib/errors"
import type { EventToastAction } from "@/lib/event-toast"
import { closeEventToasts, showEventToast, toastKeys } from "@/lib/event-toast"
import { showPushToast, showStackPushToast } from "@/lib/git/push-toast"
import { canceledBeforeStart } from "@/lib/lock-wait"
import { describeRobot, formatPushMode, openRobot } from "@/lib/patronus"
import { describePush } from "@/lib/push-summary"
import { buildRemoteMovedTexts } from "@/lib/remote-status"
import { showRobotToast } from "@/lib/robot-toast"
import { executePlan, planIntent, runFailure, sentPart, skippedReason } from "@/lib/run-intent"
import type { ToastActionProps } from "@/lib/toast-message"
import { ACTION_TOAST_TIMEOUT_MS, showToast } from "@/lib/toast-message"
import { formatReadinessSubject, formatWholeStackBranch, stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import type { Retargeted } from "@/presenters/pull-request-url"
import { formatReason, formatReasonOrNull } from "@/presenters/reasons"
import { recordPush, recordRobot } from "@/stores/activity"
import type { RemoteStatus } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, getPushOrders, getStackBranch, toRemoteStatus } from "@/stores/branch-sync"
import { whenRewritesDone } from "@/stores/operations"
import { getBranchLanding, getPushHold } from "@/stores/patronus"
import { getStackPullRequest } from "@/stores/pull-requests"
import type { PushConfirmTarget } from "@/stores/push-confirm"
import { requestPushConfirm } from "@/stores/push-confirm"
import { getRepositorySwitchReason, getSelectedRepositoryPath, isSelectedRepository, repositoryStore } from "@/stores/repository"
import { getUnappliedBranch } from "@/stores/unapplied-branches"

export interface PushBranchOptions {
  /** The remote head the push replaces: by default the row's (`RemoteStatus.remoteTip`), else what the toast of a remote that moved showed */
  readonly expectRemote?: string | null
  /** The user chose the push that drops the remote commits the branch lacks: the Force Push of the push description */
  readonly force?: boolean
  /** The Safe Push of the branch restarts on the pushed commit, canceled first while it runs (the toast of a remote that moved under a restart) */
  readonly restartRunning?: boolean
  /**
   * Whether a toast says the push went; false when the push is a step of an action that says how it went. Activity
   * records the push either way, and a failure is an error toast either way
   */
  readonly announce?: boolean
  /** The push button that asks first when the push drops remote commits: of the row (by default) or of the preview */
  readonly confirmAt?: PushConfirmTarget
}

/** How a push of a branch went */
export type PushRun =
  /** It ran: its effects in `outcome` (none when the remote had it all by then); `before`, the remote status it went over */
  | { readonly kind: "pushed"; readonly outcome: Outcome; readonly before: RemoteStatus | null }
  /** It drops remote commits the branch lacks: the push button opened its description, whose Force Push pushes */
  | { readonly kind: "confirm" }
  /** The remote branch moved since the row showed it: its toast says what it has now, and pushes over it */
  | { readonly kind: "moved" }
  /** The user stopped its wait for another holder of the repository lock: nothing was pushed, its toast said so */
  | { readonly kind: "canceled" }
  /** It did not push: its plan refuses it, or the push failed; `action` opens what the reason names */
  | { readonly kind: "failed"; readonly error: ApiError | string; readonly action?: ToastActionProps }

/** "Open in Patronus" of a refusal by a Safe Push that holds the branch or landed it */
function holdAction(branchName: string, reason: Reason): ToastActionProps | undefined {
  const hold = reason.code === "held" ? getPushHold(branchName) : null
  return hold === null ? undefined : { children: "Open in Patronus", onClick: () => openRobot(hold.robot) }
}

/**
 * Pushes the branch `branchName` alone (`runBranchPush`) and says how it went: the toast of the pushes of the branch
 * (`showPushToast`, unless `announce` is false; a restart it made in the toast of its robot then), why not in an error
 * toast; a push that asks first opened the description of its push button, a remote that moved opened its toast.
 * Resolves to whether it pushed (or found nothing left to push)
 */
export async function pushBranch(branchName: string, options: PushBranchOptions = {}): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const run = await runBranchPush(branchName, options)
  switch (run.kind) {
    case "pushed":
      announcePush(branchName, repositoryPath, run, options.announce !== false)
      return true
    case "failed":
      notifyError(`Cannot push ${branchName}`, run.error, run.action)
      return false
    case "canceled":
      showToast(canceledBeforeStart(`the push of ${branchName}`, "Nothing was pushed"))
      return false
    case "confirm":
    case "moved":
      return false
  }
}

/**
 * The push of `branchName` alone in the repository the main window shows (`Push` with the scope `alone`), over the
 * remote head its row shows unless `options` names another, after the rewrites of the repository the page runs (their
 * sync the session's: the push waits for it in Rust)
 */
export async function runBranchPush(branchName: string, options: PushBranchOptions = {}): Promise<PushRun> {
  const repositoryPath = getSelectedRepositoryPath()
  // a rewrite of the repository that runs (a move out of the branch): the push sends what it leaves
  await whenRewritesDone(repositoryPath)
  // the branches of the table are of the selected repository: another one's branch of the same name is another branch
  const switched = getRepositorySwitchReason(repositoryPath)
  if (switched !== null) {
    return { kind: "failed", error: switched }
  }
  const branch = getBranch(branchName)
  if (branch === undefined) {
    return { kind: "failed", error: `${branchName} is no longer in the branch list` }
  }
  const expectRemote = options.expectRemote === undefined ? (branch.remoteStatus?.remoteTip ?? null) : options.expectRemote
  const intent: Intent = {
    kind: "push",
    subject: { kind: "branch", name: branchName },
    scope: { kind: "alone" },
    expectRemote,
    force: options.force === true,
    restartRunning: options.restartRunning === true,
  }
  return runPush(repositoryPath, branchName, intent, { ...options, expectRemote }, branch.remoteStatus)
}

/**
 * Runs the push `intent` of `branchName` (whose remote status was `remote`) in `repositoryPath`: its plan, refused by
 * its first blocker; one that asks first opens the push button of `options.confirmAt` instead; else run on its
 * fingerprint, its progress on the push buttons of what it pushes, then what its outcome says
 */
async function runPush(repositoryPath: string, branchName: string, intent: Intent, options: PushBranchOptions, remote: RemoteStatus | null): Promise<PushRun> {
  const planned = await planIntent(repositoryPath, intent)
  switch (planned.kind) {
    case "failed":
      return { kind: "failed", error: planned.error }
    case "noPlan":
      return { kind: "failed", error: `${branchName} is no longer in the branch list` }
    case "refused":
      return { kind: "failed", error: formatReason(planned.reason, () => getBranch(branchName)?.statusText ?? ""), action: holdAction(branchName, planned.reason) }
    case "planned":
      break
  }
  // the push drops remote commits the branch lacks (`ForcePushDrops`): the user decides in the description of its button
  if (planned.plan.confirmations.length > 0) {
    requestPushConfirm(branchName, options.confirmAt ?? "row", options)
    return { kind: "confirm" }
  }
  const run = await executePlan(repositoryPath, planned.plan)
  if (run.kind !== "ran") {
    return run.kind === "failed" && getErrorCode(run.error) === "canceled"
      ? { kind: "canceled" }
      : { kind: "failed", error: runFailure(run, `${branchName} is no longer in the branch list`) ?? "" }
  }
  const outcome = run.outcome
  for (const { effect, status } of outcome.effects) {
    if (effect.kind === "retarget" && status.kind === "failed") {
      // the push went on without it: its toast offers the command
      notifyError(`Cannot retarget pull request #${effect.number}`, status.error)
    }
  }
  const push = outcome.effects.find(({ effect }) => effect.kind === "push")
  if (push?.status.kind === "failed") {
    if (getErrorCode(push.status.error) === "lease_rejected" && intent.kind === "push" && intent.scope.kind === "alone") {
      // the session has what the remote has now, fetched after the refusal
      const moved = getBranch(branchName)?.remoteStatus ?? null
      if (moved !== null) {
        recordPush(repositoryPath, branchName, remote, moved)
        await showRemoteMovedToast(repositoryPath, branchName, moved, intent.restartRunning)
        return { kind: "moved" }
      }
    }
    return { kind: "failed", error: push.status.error }
  }
  const failed = outcome.effects.find(({ effect, status }) => status.kind === "failed" && effect.kind !== "retarget")
  if (failed?.status.kind === "failed") {
    return { kind: "failed", error: failed.status.error }
  }
  const skipped = skippedReason(outcome)
  if (skipped !== null) {
    return { kind: "failed", error: skipped }
  }
  const over = outcome.result.kind === "push" ? outcome.result.pushedOver[0] : undefined
  return { kind: "pushed", outcome, before: over === undefined ? remote : toRemoteStatus(over) }
}

/**
 * Says how the push of `branchName` went: in the toast of the pushes of the branch (`announce`), with the pull request it
 * retargeted first and the Safe Push it restarted; a push that is a step of an action says only the restart, in the
 * toast of its robot. A run that pushed nothing (the remote had it all by then) says nothing
 */
function announcePush(branchName: string, repositoryPath: string, run: Extract<PushRun, { kind: "pushed" }>, announce: boolean): void {
  const { outcome } = run
  const restarted = restartedOf(outcome, repositoryPath)
  const pushed = outcome.effects.some(({ effect, status }) => effect.kind === "push" && status.kind === "done")
  if (!announce || !pushed) {
    if (restarted !== undefined) {
      showRobotToast(`${formatPushMode(restarted.pushMode)} restarted`, restarted, repositoryPath)
    }
    return
  }
  showPushToast(branchName, repositoryPath, describePush(branchName, run.before), restarted, retargetedOf(outcome))
}

/** The Safe Push the push restarted on the pushed commit, recorded in Activity of `repositoryPath`; undefined for none */
function restartedOf(outcome: Outcome, repositoryPath: string): PatronusRobot | undefined {
  const restarted = outcome.result.kind === "push" ? outcome.result.restarted : null
  if (restarted == null) {
    return undefined
  }
  recordRobot(`${formatPushMode(restarted.pushMode)} restarted`, describeRobot(restarted), restarted, repositoryPath, outcome.op)
  return restarted
}

/** The pull request the push retargeted first (of `branch`, else of any branch it sent), as its toast names it */
function retargetedOf(outcome: Outcome, branch?: string): Retargeted | undefined {
  for (const { effect, status } of outcome.effects) {
    if (effect.kind === "retarget" && status.kind === "done" && (branch === undefined || effect.branch === branch)) {
      return { number: effect.number, base: effect.to }
    }
  }
  return undefined
}

/**
 * Pushes the unapplied branch `name` (its full name) to the remote under its own name, so the remote keeps a copy
 * (`Push` with the scope `unapplied`): leased on the remote head of its status (none: the remote branch must not exist),
 * with no robot read and no sync; its row shows the remote status the push left. It lands nothing; a pull request of
 * its virtual branch stays as it was. "Force Push" of its menu is the user's choice to drop what the remote has
 */
export async function pushUnappliedBranch(name: string): Promise<void> {
  const branch = getUnappliedBranch(name)
  if (branch === undefined) {
    return
  }
  const repositoryPath = getSelectedRepositoryPath()
  const intent: Intent = {
    kind: "push",
    subject: { kind: "branch", name: branch.branch },
    scope: { kind: "unapplied" },
    expectRemote: branch.remoteStatus?.remoteTip ?? null,
    force: branch.remoteStatus?.action.kind === "force",
    restartRunning: false,
  }
  const run = await runPush(repositoryPath, branch.branch, intent, {}, branch.remoteStatus)
  if (run.kind === "pushed") {
    showToast({ title: `Pushed ${branch.branch}`, description: `The remote keeps it as ${name}` })
  } else if (run.kind === "failed") {
    const moved = typeof run.error !== "string" && getErrorCode(run.error) === "lease_rejected"
    notifyError(
      `Cannot push ${branch.branch}`,
      moved ? `The remote branch ${name} changed since the last fetch: nothing was pushed. Its status shows it now; a push again replaces it` : run.error,
    )
  } else if (run.kind === "canceled") {
    showToast(canceledBeforeStart(`the push of ${branch.branch}`, "Nothing was pushed"))
  }
}

/**
 * The remote branch moved since its status was shown, so nothing was pushed: the toast says what the remote has now,
 * and its action pushes over exactly that, through the plan of the push as any push (one that drops the commits the
 * remote has now asks in the row first). A push under a restart (`restartRunning`) restarts the Safe Push of the branch
 * on what it pushes; with nothing left to push it restarts it at once, and the toast says so and opens it. The toast is
 * of the repository the push went to: it offers its push only while the main window shows that one (the branch of the
 * same name in another one is another branch), and closes once the branch is gone; a push of the branch takes its place.
 */
export async function showRemoteMovedToast(repositoryPath: string, branchName: string, remoteStatus: RemoteStatus, restartRunning: boolean): Promise<void> {
  const texts = buildRemoteMovedTexts(branchName, remoteStatus)
  // nothing left to push: the remote has the branch's commits, the robot can run on them
  let restarted: PatronusRobot | undefined
  if (restartRunning && texts.action === null) {
    const run = await runBranchPush(branchName, { expectRemote: remoteStatus.remoteTip, restartRunning: true })
    restarted = run.kind === "pushed" ? restartedOf(run.outcome, repositoryPath) : undefined
    if (run.kind === "failed") {
      notifyError("Cannot restart the Safe Push", run.error)
    }
  }
  let note = ""
  if (restarted !== undefined) {
    note = ` The ${formatPushMode(restarted.pushMode)} restarted on it.`
  } else if (restartRunning && texts.action !== null) {
    note = " The Safe Push is canceled: the push restarts it."
  }
  const key = toastKeys.push(repositoryPath, branchName)
  const description = `${texts.description}${note}`
  let action: EventToastAction | undefined
  if (texts.action !== null) {
    action = {
      label: texts.action,
      onClick: () => {
        closeEventToasts(key)
        const switched = getRepositorySwitchReason(repositoryPath)
        if (switched === null) {
          void pushBranch(branchName, { expectRemote: remoteStatus.remoteTip, restartRunning, confirmAt: "row" })
        } else {
          notifyError(`Cannot push ${branchName}`, switched)
        }
      },
    }
  } else if (restarted !== undefined) {
    const robot = restarted
    action = { label: "Open in Patronus", onClick: () => openRobot(robot) }
  }
  // the toast of the pushes of the branch: a push of it since takes its place. Its push only while the main window shows
  // the repository, whose branch it pushes (the branch of the same name in another one is another branch), and it closes
  // once the branch is gone
  showEventToast({
    key,
    watch: [branchSyncStore, repositoryStore],
    render: () => {
      if (getRepositorySwitchReason(repositoryPath) !== null) {
        return { type: "warning", title: texts.title, description, ...(texts.action === null && { action }), timeout: ACTION_TOAST_TIMEOUT_MS }
      }
      // the user decides: the toast stays until then
      return getBranch(branchName) === undefined
        ? null
        : { type: "warning", title: texts.title, description, action, timeout: texts.action === null && restarted !== undefined ? ACTION_TOAST_TIMEOUT_MS : 0 }
    },
  })
}

/** A push of a stack in the words of the surface that ran it */
export interface StackPushOptions {
  /** Whether a toast says what was pushed; false when the push is a step of an action that says how it went */
  readonly announce?: boolean
}

/**
 * What a click on the push button of the branch pushes (`confirmAt` the button, which asks first when the push drops
 * remote commits): the stack while the view says the button pushes it (`PushOrders.button`: an ancestor has work and
 * the branch asks nothing), else the branch alone (`pushBranch`). The same push runs from the branch menu and P
 */
export function pushFromButton(branchName: string, confirmAt: PushConfirmTarget = "row"): void {
  if (getPushOrders(branchName).button.length > 0) {
    void runStackPush(branchName, stackIntent(branchName, { kind: "withAncestors" }, false), {})
  } else {
    void pushBranch(branchName, { confirmAt })
  }
}

/**
 * Push Stack of the menu and of the description of the push button: the branches it is built on that have work for
 * the remote up to the nearest one a Safe Push holds, then the branch, in one atomic push (`Push` with the scope
 * `withAncestors`, forced: the description named each push that drops remote commits)
 */
export function pushStack(branchName: string): Promise<boolean> {
  return runStackPush(branchName, stackIntent(branchName, { kind: "withAncestors" }, true), {})
}

/**
 * Push with Children: the branch while it has work, the branches stacked on it that have work, and for the root of a
 * stack that forks its stack branch while it goes along, in one atomic push (`withChildren`), so the pull request of the
 * whole stack follows its branches
 */
export function pushWithChildren(branchName: string): Promise<boolean> {
  return runStackPush(branchName, stackIntent(branchName, { kind: "withChildren" }, false), {})
}

/** Push Up to Here of the commit `commit` (its copy) of `branchName`: what it is built on first, whole, then the branch up to it (`upTo`) */
export function pushUpTo(branchName: string, commit: string): Promise<boolean> {
  return runStackPush(branchName, stackIntent(branchName, { kind: "upTo", commit }, false), {})
}

/**
 * The push of the stack branch of the whole stack of `root` alone (the push a refused start of the whole stack offers):
 * the build of the last sync while its remote lacks it; what holds it refuses it. No toast: the action it is a step
 * of says it; Activity records the push
 */
export function pushWholeStack(root: string): Promise<boolean> {
  const intent: Intent = {
    kind: "push",
    subject: { kind: "wholeStack", root },
    scope: { kind: "alone" },
    expectRemote: getStackBranch(root)?.remoteStatus?.remoteTip ?? null,
    force: false,
    restartRunning: false,
  }
  return runStackPush(stackReadinessKey(root), intent, { announce: false })
}

/**
 * The push an action needs before it runs ("Push" of the toast of a start that can't run): what the push button sends,
 * without a toast of its own; one the user decides (it drops remote commits of the branch or of an ancestor, a branch
 * of its chain the sync did not build) refuses with the reason, as Rust words it of the view
 * (`domain_policy::holds::push_blocker`). Resolves to whether it pushed
 */
export async function pushForAction(branchName: string): Promise<boolean> {
  const blocker = formatReasonOrNull(getBranchLanding(branchName).push, name => getBranch(name)?.statusText ?? "")
  if (blocker !== null) {
    notifyError(`Cannot push ${branchName}`, blocker)
    return false
  }
  return getPushOrders(branchName).button.length > 0
    ? runStackPush(branchName, stackIntent(branchName, { kind: "withAncestors" }, false), { announce: false })
    : pushBranch(branchName, { announce: false })
}

/** The intent of a push of a stack from `branchName` of `scope`, over the remote head its row shows */
function stackIntent(branchName: string, scope: PushScope, force: boolean): Intent {
  return {
    kind: "push",
    subject: { kind: "branch", name: branchName },
    scope,
    expectRemote: getBranch(branchName)?.remoteStatus?.remoteTip ?? null,
    force,
    restartRunning: false,
  }
}

/** The remote status a name of the push effect of a plan had before it: of a branch, of a stack branch (`stack:<root>`) */
function remoteStatusOf(name: string): RemoteStatus | null {
  const root = stackRootOfReadinessKey(name)
  return (root === null ? getBranch(name)?.remoteStatus : getStackBranch(root)?.remoteStatus) ?? null
}

/**
 * Runs the push `intent` of a stack started from `startedFrom` in the repository the main window shows, after its
 * rewrites (their sync the session's: the push waits for it in Rust): its plan, refused by its first blocker, run with
 * what it asks accepted (the surface that offered it described each push that drops remote commits), its one atomic
 * push showing its progress on the push buttons of what it sends. What its outcome says: a retarget that failed, the
 * remote that moved (its toast, or of a stack branch why it isn't pushed), the toast of what it pushed
 * (`showStackPushToast`, unless `announce` is false). Resolves to whether it pushed, true when nothing was left
 */
async function runStackPush(startedFrom: string, intent: Intent, options: StackPushOptions): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const failure = `Cannot push ${formatReadinessSubject(startedFrom)}`
  // a rewrite of the repository that runs (a move out of a branch): the push sends what it leaves
  await whenRewritesDone(repositoryPath)
  const switched = getRepositorySwitchReason(repositoryPath)
  if (switched !== null) {
    notifyError(failure, switched)
    return false
  }
  const planned = await planIntent(repositoryPath, intent)
  if (planned.kind !== "planned") {
    const reason =
      planned.kind === "refused" ? formatReason(planned.reason, name => getBranch(name)?.statusText ?? "") : runFailure(planned, `${startedFrom} is no longer in the branch list`)
    notifyError(failure, reason ?? "", planned.kind === "refused" ? holdAction(startedFrom, planned.reason) : undefined)
    return false
  }
  const push = planned.plan.effects.find(effect => effect.kind === "push")
  if (push?.kind !== "push") {
    return true
  }
  const before = new Map(push.branches.map(name => [name, remoteStatusOf(name)] as const))
  const run = await executePlan(repositoryPath, planned.plan)
  if (run.kind !== "ran") {
    if (run.kind === "failed" && getErrorCode(run.error) === "canceled") {
      showToast(canceledBeforeStart(`the push of ${formatReadinessSubject(startedFrom)}`, "Nothing was pushed"))
    } else {
      notifyError(failure, runFailure(run, `${startedFrom} is no longer in the branch list`) ?? "")
    }
    return false
  }
  return await toldStackPush(startedFrom, repositoryPath, run.outcome, before, intent, options)
}

/** What the outcome of a push of a stack says (`runStackPush`); whether it pushed */
async function toldStackPush(
  startedFrom: string,
  repositoryPath: string,
  outcome: Outcome,
  before: ReadonlyMap<string, RemoteStatus | null>,
  intent: Intent,
  options: StackPushOptions,
): Promise<boolean> {
  const failure = `Cannot push ${formatReadinessSubject(startedFrom)}`
  for (const { effect, status } of outcome.effects) {
    if (effect.kind === "retarget" && status.kind === "failed") {
      // the push went on without it: its toast offers the command
      notifyError(`Cannot retarget pull request #${effect.number}`, status.error)
    }
  }
  const record = outcome.effects.find(({ effect }) => effect.kind === "push")
  if (record?.status.kind === "failed") {
    const error = record.status.error
    const moved = getErrorCode(error) === "lease_rejected" ? movedOf(error) : null
    const root = moved === null ? null : stackRootOfReadinessKey(moved)
    if (moved !== null && root === null && getBranch(moved)?.remoteStatus != null) {
      // the session has what the remote has now, fetched after the refusal
      await showRemoteMovedToast(repositoryPath, moved, getBranch(moved)!.remoteStatus!, false)
    } else if (root !== null) {
      notifyError(`Cannot push ${formatWholeStackBranch(root)}`, describeMovedStackBranch(getStackBranch(root)?.remoteStatus ?? null))
    } else {
      notifyError(failure, error)
    }
    return false
  }
  const skipped = skippedReason(outcome)
  if (skipped !== null) {
    notifyError(failure, skipped)
    return false
  }
  if (record === undefined || options.announce === false || record.effect.kind !== "push") {
    return true
  }
  const over = outcome.result.kind === "push" ? outcome.result.pushedOver : []
  const tip = intent.kind === "push" && intent.scope.kind === "upTo" ? intent.scope.commit : undefined
  const sent = record.effect.branches.map((name, at) => ({ name, before: over[at] === undefined ? (before.get(name) ?? null) : toRemoteStatus(over[at]) }))
  const outcomes = sent
    .filter(it => stackRootOfReadinessKey(it.name) === null)
    .map(it => ({ name: it.name, before: it.name === startedFrom ? sentPart(it.before, tip) : it.before, retargeted: retargetedOf(outcome, it.name) }))
  const stack = sent.find(it => stackRootOfReadinessKey(it.name) !== null)
  const root = stack === undefined ? null : stackRootOfReadinessKey(stack.name)
  // after a switch the pull requests of the table are of the other repository: the toast offers none
  const pullRequest = root === null || !isSelectedRepository(repositoryPath) ? undefined : getStackPullRequest(root)
  showStackPushToast(outcomes, startedFrom, repositoryPath, root === null ? null : { root, before: stack?.before ?? null, pullRequest })
  return true
}

/** The branch (`stack:<root>` the stack branch) whose remote moved, as the failed push names it (`ErrorDetails.branches`) */
function movedOf(error: ApiError): string | null {
  const first = error.details?.type === "branches" ? error.details.branches[0] : undefined
  if (first === undefined) {
    return null
  }
  const stack = first.indexOf("/stack/")
  if (stack >= 0) {
    return stackReadinessKey(first.slice(stack + "/stack/".length))
  }
  const virtual = first.indexOf("/virtual/")
  return virtual >= 0 ? first.slice(virtual + "/virtual/".length) : first
}

/** Why a stack branch whose remote moved isn't pushed: its remote has commits it lacks, and the next sync rebuilds it */
function describeMovedStackBranch(status: RemoteStatus | null): string {
  const behind = status?.commitsBehind ?? 0
  return behind === 0
    ? "Its remote branch moved since its status was shown: sync, then push again"
    : `Its remote branch has ${behind === 1 ? "a commit" : `${behind} commits`} the stack branch lacks: sync, then push again`
}
