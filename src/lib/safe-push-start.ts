import { restackBranch } from "@/hooks/useStackActions"
import type { LandingAllowances, PatronusMergeRequest, PatronusRobot, PlannedEffect } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { showEventToast, toastKeys } from "@/lib/event-toast"
import { cancelHoldingRobot, pushForAction } from "@/lib/git/push"
import { pushWholeStack } from "@/lib/git/stack-push"
import type { LandingDecision } from "@/lib/landing-gate"
import { checkLandingBeforeStart, formatLandingFix } from "@/lib/landing-gate"
import { describeRobot, formatPushMode } from "@/lib/patronus"
import { getRobotActionItems } from "@/lib/patronus-actions"
import { openPullRequestUrl } from "@/lib/pull-request"
import { showRobotEventToast } from "@/lib/robot-toast"
import { checkTargetBeforeStart } from "@/lib/target-conflict"
import { recordActivity, recordRetarget, recordRobot } from "@/stores/activity"
import { branchSyncStore } from "@/stores/branch-sync"
import type { StartRepository } from "@/stores/patronus"
import { getRobot, startPatronusFromPullRequest, startPatronusRobot } from "@/stores/patronus"
import { getPullRequest, retargetPullRequest } from "@/stores/pull-requests"
import { getRepositorySwitchReason, vcsRequestFactory } from "@/stores/repository"
import type { SafePushMode, SafePushStarting } from "@/stores/safe-push"
import { getSafePushStarting, setSafePushStarting } from "@/stores/safe-push"
import { runUpdateProjectOf } from "@/stores/update-project"

/** What a start runs, as Rust planned it (`Plan.effects` of `prepare`) and the user confirmed */
export interface SafePushStart {
  readonly branchName: string
  readonly mode: SafePushMode
  /**
   * What the start does once nothing refuses it, in order (`PlannedEffect`): the retargets of the pull requests into a
   * virtual branch (D11), the cancel of the Safe Push of an older commit it replaces, the push of what the remote lacks
   * (of the whole stack its stack branch), then the robot, through the API or from a pull request
   */
  readonly effects: readonly PlannedEffect[]
  /** The merge profile of a start through the API; null: the default checks */
  readonly profile: { readonly id: string; readonly name: string } | null
  /** The open merge request of a repository only on Space, whose approvals count for the start */
  readonly mergeRequest: PatronusMergeRequest | null
  /**
   * The root of the whole stack it lands (`branchName` is its key, `stackReadinessKey`): the robot starts from its stack
   * branch (`stack`), the only branch pushed first; null for the start of a branch
   */
  readonly wholeStack: string | null
  /** What the user starts anyway (Start Anyway of a refusal of the landing gate); absent: every refusal holds it back */
  readonly allow?: LandingAllowances
}

/**
 * Starts a Safe Push or a Dry Run: first the landing gate (`checkLandingBeforeStart`, the rule of `bd safe-push`: a name
 * what lands uses and a branch outside it adds, a proven dangling reference), whose refusal starts nothing and offers the
 * fix and Start Anyway (`refuseLanding`); then the target conflict check (`checkTargetBeforeStart`: the fetch of the
 * target and the replay of what lands onto its tip, as Patronus rebases it), whose conflict starts nothing and says
 * "Cannot start the Safe Push" with Update Project; then the effects Rust planned, in their order: the retarget of each
 * pull request into a virtual branch into the target (D11), the push of what the remote lacks (`pushForAction`: the
 * stack root first, a running Safe Push of an older commit canceled for it; of the whole stack its stack branch alone)
 * or, without a push, the cancel of that Safe Push, then the robot, from the pull request (a `/patronus` comment) or
 * through the API with the profile. Its steps show in the button of the slot (`SafePushStarting`); the one toast of the
 * action is the start's ("Safe Push started", with Open in Patronus and Cancel Safe Push, and a sentence when the check
 * did not run or used the last fetched target), the push has none (Activity records it). A failure is "Cannot start the
 * Safe Push" with the reason; a failed retarget or push starts nothing. A comment Patronus accepted without a listed
 * robot keeps the slot "Starting…" until the robot is listed (`holdUntilListed`). Resolves to whether the robot started.
 */
export async function startSafePush(start: SafePushStart, repository: StartRepository | null = null): Promise<boolean> {
  const { branchName, mode } = start
  const modeName = formatPushMode(mode)
  const errorTitle = `Cannot start the ${modeName}`
  // the repository the start was asked in, frozen at admission: every step names it, so a switch of the repository
  // meanwhile starts nothing in the other one
  const admitted = repository ?? admitStart(errorTitle)
  if (admitted === null) {
    return false
  }
  const { request } = admitted
  const repositoryPath = request.repositoryPath
  const current = getSafePushStarting(branchName, repositoryPath)
  if (current !== null && current.step !== "checking") {
    return false
  }
  const root = start.wholeStack
  const setStep = (step: SafePushStarting["step"] | null) => setSafePushStarting(branchName, step === null ? null : { mode, step }, repositoryPath)
  // an accepted comment whose robot Patronus lists not yet keeps the start until it does (`holdUntilListed`)
  let held = false
  try {
    setStep("target")
    const allow = start.allow ?? {}
    const landing = await checkLandingBeforeStart(branchName, root, allow, admitted)
    if (landing.kind === "refuse") {
      refuseLanding(start, errorTitle, landing, admitted)
      return false
    }
    const target = await checkTargetBeforeStart(branchName, root, admitted)
    if (target.kind === "refuse") {
      // the update of the repository of the start: a fetch only, once the main window shows another one
      notifyError(errorTitle, target.reason, { children: "Update Project", onClick: () => void runUpdateProjectOf(repositoryPath) })
      return false
    }
    // the retargets, the push and the robot read and change the pull requests and the branches of the table, which are
    // of the selected repository
    const push = start.effects.find(effect => effect.kind === "push")
    const pushes = push !== undefined
    const retargets = start.effects.filter(effect => effect.kind === "retarget")
    if (pushes || retargets.length > 0) {
      const switched = getRepositorySwitchReason(repositoryPath)
      if (switched !== null) {
        notifyError(errorTitle, switched)
        return false
      }
    }
    for (const retarget of retargets) {
      await retargetForStart(retarget)
    }
    const cancel = start.effects.find(effect => effect.kind === "cancelRobot")
    const replaces = cancel?.kind === "cancelRobot" ? (getRobot(cancel.robotId) ?? null) : null
    if (push?.kind === "push") {
      setStep("pushing")
      if (!(await pushFirst(start, push.branches, replaces, errorTitle, repositoryPath))) {
        return false
      }
    } else if (replaces !== null && !(await cancelHoldingRobot(replaces, notifyError, request))) {
      return false
    }
    const route = start.effects.find(effect => effect.kind === "startRobot")?.route
    if (route === undefined) {
      return false
    }
    setStep("starting")
    const note = [landing.note, target.note].filter(line => line !== null).join(" ")
    const title = `${modeName} started`
    if (route.kind === "api") {
      const robot = await startPatronusRobot(root ?? branchName, mode, start.profile?.id ?? null, start.mergeRequest, start.profile?.name ?? null, root !== null, allow, admitted)
      announceStart(title, robot, start.profile?.name ?? null, note === "" ? null : note, repositoryPath)
      return true
    }
    const number = route.number
    const started = await startPatronusFromPullRequest(number, mode, { branchName: root ?? branchName, stack: root !== null, allow }, request)
    if (started.robot === null) {
      announceCommentStart(start, title, number, started.commentUrl, note === "" ? null : note, repositoryPath)
      held = holdUntilListed(branchName, mode, started.listed, repositoryPath)
    } else {
      announceStart(title, started.robot, null, note === "" ? null : note, repositoryPath)
    }
    return true
  } catch (error) {
    notifyError(errorTitle, error)
    return false
  } finally {
    if (!held) {
      setStep(null)
    }
  }
}

/**
 * The retarget of a pull request into a virtual branch into the target before the start (D11): every pull request of a
 * virtual branch goes into the target, and the robot starts from it there. Activity records it; a failure throws, so the
 * start stops
 */
async function retargetForStart(retarget: Extract<PlannedEffect, { kind: "retarget" }>): Promise<void> {
  await retargetPullRequest(retarget.branch, retarget.to)
  const url = getPullRequest(retarget.branch)?.url
  if (url !== undefined) {
    recordRetarget(`Retargeted #${retarget.number} to ${retarget.to}`, `${retarget.branch} · it went into ${retarget.from}`, url)
  }
}

/**
 * The repository a start is asked in, as the main window shows it now, and its baseline; null when none is configured
 * (the error toast said so under `errorTitle`)
 */
export function admitStart(errorTitle: string): StartRepository | null {
  try {
    return { request: vcsRequestFactory.createRequest(), baselineBranch: branchSyncStore.getState().baselineBranch ?? "" }
  } catch (error) {
    notifyError(errorTitle, error)
    return null
  }
}

/**
 * Keeps the start of `branchName` in `repositoryPath` "Starting…" while Patronus lists not yet the robot of the comment
 * it accepted: the row offers no second start, whose check would see no robot and post a second `/patronus` comment.
 * The slot is the repository's own (`setSafePushStarting`): a switch to another repository shows nothing on its branch
 * of the same name, and the hold is there again on the way back. Ends once `listed` settles (the robot holds the next
 * start back then, or none came within `LATE_ROBOT_WAIT_MS`). Returns true.
 */
function holdUntilListed(branchName: string, mode: SafePushMode, listed: Promise<unknown>, repositoryPath: string): true {
  setSafePushStarting(branchName, { mode, step: "starting" }, repositoryPath)
  const release = () => {
    if (getSafePushStarting(branchName, repositoryPath)?.step === "starting") {
      setSafePushStarting(branchName, null, repositoryPath)
    }
  }
  void listed.then(release, release)
  return true
}

/**
 * A start the landing gate refused: "Cannot start the Safe Push" with why the build of what lands fails, the fix of the
 * first refusal as the action ("Stack lane-run on stills-codec", a restack the start then lands too), and Start Anyway,
 * the same start with the overrides of what refused it (a name the user knows is a false alarm)
 */
function refuseLanding(start: SafePushStart, errorTitle: string, landing: Extract<LandingDecision, { kind: "refuse" }>, repository: StartRepository): void {
  const anyway = { label: "Start Anyway", onClick: () => void startSafePush({ ...start, allow: landing.anyway }, repository) }
  const fix = landing.fix
  const fixLabel = formatLandingFix(fix)
  if (fix?.kind === "restack" && fixLabel !== null) {
    notifyError(errorTitle, landing.reason, { children: fixLabel, onClick: () => void restackBranch(fix.branch, fix.onto) }, anyway)
  } else {
    notifyError(errorTitle, landing.reason, { children: anyway.label, onClick: anyway.onClick })
  }
}

/**
 * The push of what the remote lacks before the start, as its decision pushes it (`branches` of its `push` effect): the
 * branch and the ancestors it lands with it (`pushForAction`), or only the stack branch of the whole stack
 * (`pushWholeStack`); a failure said under `errorTitle`. Resolves to whether it was pushed.
 */
function pushFirst(start: SafePushStart, branches: readonly string[], replaces: PatronusRobot | null, errorTitle: string, repositoryPath: string): Promise<boolean> {
  const replacesRobot = replaces ?? undefined
  if (start.wholeStack !== null) {
    return pushWholeStack(start.wholeStack, { replacesRobot, report: (_title, reason) => notifyError(errorTitle, reason) })
  }
  return pushForAction(start.branchName, { replacesRobot, repositoryPath, branches }).then(pushed => {
    // a push stopped while it waited for the lock said so itself: the start fails nothing
    if (!pushed.pushed && pushed.canceled !== true) {
      notifyError(errorTitle, pushed.reason)
    }
    return pushed.pushed
  })
}

/**
 * The toast of a start Patronus accepted on pull request `number` (👍) whose robot it lists not yet: the start is made,
 * the robot shows once listed (looked for in the background); the toast opens the comment. It is the toast of the
 * robots of the subject (`robotToastKeys`): it takes the place of the toast of the robot it restarts, or of the
 * approvals it starts on
 */
function announceCommentStart(start: SafePushStart, title: string, number: number, commentUrl: string, note: string | null, repositoryPath: string): void {
  const accepted = `Patronus accepted the comment on #${number}: the robot shows once Patronus lists it`
  const description = note === null ? accepted : `${accepted}. ${note}`
  recordActivity({ repositoryPath, kind: "patronus", title, description, details: { type: "pullRequest", url: commentUrl } })
  // the key of the subject: the branch, or the key of the whole stack (`stackReadinessKey`), as `robotToastSubject`
  const subject = toastKeys.robotSubject(repositoryPath, start.branchName, start.mode)
  showEventToast({
    key: subject,
    supersedes: start.mode === "dryRun" ? [] : [toastKeys.approval(repositoryPath, start.branchName)],
    render: () => ({ type: "success", title, description, action: { label: `Open Pull Request #${number}`, onClick: () => openPullRequestUrl(commentUrl) } }),
  })
}

/**
 * The toast of a started robot, as `announceRobot` says it, with "Cancel Safe Push" below the text while the robot can
 * be canceled: a start asked by mistake costs only the robot (the push and the comment stay); Activity records it.
 * `note` follows the description: the target conflict check did not run, or used the last fetched target. The toast
 * follows the robot (`showRobotEventToast`): its failure or its merge take its place
 */
function announceStart(title: string, robot: PatronusRobot, profileName: string | null, note: string | null, repositoryPath: string): void {
  const description = note === null ? describeRobot(robot, profileName) : `${describeRobot(robot, profileName)}. ${note}`
  recordRobot(title, description, robot, repositoryPath)
  showRobotEventToast({
    repositoryPath,
    robot,
    type: "success",
    title,
    description,
    stepLast: true,
    stepOf: current => getRobotActionItems(current).find(action => action.id === "cancel-robot") ?? null,
  })
}
