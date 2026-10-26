// The restart and the cancel of a robot from every surface that offers them (the robot menu, its details, the robots
// indicator, a toast, the hold of a push): the intents `RestartRobot` and `CancelRobot` Rust plans and runs
// (`operations::robot`; spec/branch/patronus.spec.md, "Restart and cancel"). A restart pushes first while the branch has
// work its remote lacks, the running robot canceled before the push; a cancel of a robot that finished meanwhile is
// answered as it ended. The page only words how it went: the next state of the toast of the robot ("Safe Push
// restarted", "Safe Push is canceling"), the remote that moved with the push that restarts it, or why not.
import type { OperationEvent, Outcome, PatronusRobot, RobotResult } from "@/lib/bindings"
import { getErrorCode, notifyError } from "@/lib/errors"
import { showRemoteMovedToast } from "@/lib/git/push"
import { describeRobot, formatPushMode, formatRobotSubject } from "@/lib/patronus"
import { showRobotToast } from "@/lib/robot-toast"
import type { IntentRun } from "@/lib/run-intent"
import { failedEffect, runFailure, runIntent } from "@/lib/run-intent"
import { showToast } from "@/lib/toast-message"
import { stackReadinessKey } from "@/lib/whole-stack"
import { recordRobot } from "@/stores/activity"
import { getBranch, setBranchPushing, setBranchPushProgress } from "@/stores/branch-sync"
import { getSelectedRepositoryPath } from "@/stores/repository"

/**
 * Says that a robot restarted or is canceling, in the toast of the robot and in Activity of `repositoryPath`: "Safe Push
 * restarted" / "auth → master", with "Open in Patronus" (a toast about an object opens it)
 */
export function announceRobot(title: string, robot: PatronusRobot, repositoryPath: string, operationId?: string): void {
  showRobotToast(title, robot, repositoryPath)
  recordRobot(title, describeRobot(robot), robot, repositoryPath, operationId)
}

/** What a restart or a cancel answered of the robot, when it ran */
function robotResult(outcome: Outcome): RobotResult | null {
  return outcome.result.kind === "robot" ? outcome.result : null
}

/** The robot a restart started (`run` of `RestartRobot`); null when it started none */
export function restartedOf(run: IntentRun): PatronusRobot | null {
  return run.kind === "ran" ? (robotResult(run.outcome)?.robot ?? null) : null
}

/**
 * Says why a restart or a cancel of a robot did not get there under `title`: what its plan refuses and the facts that
 * changed before its effect in an info toast (nothing failed, the user decides again), what failed in an error toast
 */
function tellFailure(title: string, run: IntentRun, noPlan: string): void {
  const failure = runFailure(run, noPlan)
  const failed = run.kind === "failed" || (run.kind === "ran" && failedEffect(run.outcome) !== null)
  if (failed && failure !== null) {
    notifyError(title, failure)
  } else {
    showToast({ type: "info", title, description: typeof failure === "string" ? failure : (failure?.message ?? noPlan) })
  }
}

/**
 * What the push buttons show of the push of a restart while it sends (the step `pushing` of its run): the branch of the
 * robot, or the stack branch of its whole stack ("Pushing the whole stack 45%"), with the percent and the line of git;
 * `done` clears it
 */
function pushShown(robot: PatronusRobot, repositoryPath: string): { readonly onEvent: (event: OperationEvent) => void; readonly done: () => void } {
  const key = robot.branchName ?? (robot.stackRoot == null ? null : stackReadinessKey(robot.stackRoot))
  let pushing = false
  const show = (now: boolean) => {
    if (key !== null && now !== pushing) {
      pushing = now
      setBranchPushing(key, now, repositoryPath)
    }
  }
  const onEvent = (event: OperationEvent) => {
    if (event.kind === "step") {
      show(event.step.kind === "pushing")
    } else if (event.kind === "progress" && pushing && key !== null) {
      setBranchPushProgress(key, { percent: event.percent, line: event.line, transferred: null, speed: null }, repositoryPath)
    }
  }
  return { onEvent, done: () => show(false) }
}

/**
 * Restarts `robot` in `repositoryPath` (the repository the main window shows, where its action ran): in the toast of
 * the robot "Safe Push restarted", in place of its own (a failed robot's toast becomes the restart); the push it makes
 * first shows on the push button of its branch or of its whole stack. A push the remote refused because the remote
 * branch moved meanwhile opens the toast of the moved remote, whose push restarts the robot (it is canceled then); a
 * refusal, a failure or a change of the facts is said in a toast. Resolves to the new robot
 */
export async function restartRobot(robot: PatronusRobot, repositoryPath: string = getSelectedRepositoryPath()): Promise<PatronusRobot | null> {
  const mode = formatPushMode(robot.pushMode)
  const push = pushShown(robot, repositoryPath)
  const run = await runIntent(repositoryPath, { kind: "restartRobot", robotId: robot.id }, push.onEvent).finally(push.done)
  const restarted = restartedOf(run)
  if (restarted !== null) {
    announceRobot(`${mode} restarted`, restarted, repositoryPath, run.kind === "ran" ? run.outcome.op : undefined)
    return restarted
  }
  const failed = run.kind === "ran" ? failedEffect(run.outcome) : null
  const moved = robot.branchName === null ? undefined : getBranch(robot.branchName)?.remoteStatus
  if (failed?.effect.kind === "push" && getErrorCode(failed.error) === "lease_rejected" && robot.branchName !== null && moved != null) {
    await showRemoteMovedToast({ repositoryPath }, robot.branchName, moved, robot)
    return null
  }
  tellFailure(`Cannot restart the ${mode}`, run, `Patronus offers no restart of ${formatRobotSubject(robot)} now`)
  return null
}

/**
 * Cancels `robot` in `repositoryPath`: in the toast of the robot "Safe Push is canceling"; one that finished meanwhile
 * is answered as it ended ("Safe Push already finished"), nothing canceled. Resolves to the robot as the cancel left it
 */
export async function cancelRobot(robot: PatronusRobot, repositoryPath: string = getSelectedRepositoryPath()): Promise<PatronusRobot | null> {
  const mode = formatPushMode(robot.pushMode)
  const run = await runIntent(repositoryPath, { kind: "cancelRobot", robotId: robot.id })
  const result = run.kind === "ran" && failedEffect(run.outcome) === null ? robotResult(run.outcome) : null
  if (result?.robot == null) {
    tellFailure(`Cannot cancel the ${mode}`, run, `Patronus lists no running ${mode} of ${formatRobotSubject(robot)}`)
    return null
  }
  announceRobot(result.alreadyFinished ? `${mode} already finished` : `${mode} is canceling`, result.robot, repositoryPath, run.kind === "ran" ? run.outcome.op : undefined)
  return result.robot
}
