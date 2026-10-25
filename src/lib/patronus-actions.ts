import { ClipboardCopyIcon, ClipboardListIcon, ExternalLinkIcon, GitPullRequestIcon, RotateCcwIcon, ShieldOffIcon } from "lucide-react"
import type { PatronusRobot } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { pushBranch } from "@/lib/git/push"
import { pushWholeStack } from "@/lib/git/stack-push"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem } from "@/lib/menu"
import { canCancelRobot, describeRobot, formatPushMode, formatRobotRestartLabel, formatRobotSubject, openRobot } from "@/lib/patronus"
import { openPullRequestUrl } from "@/lib/pull-request"
import { copyRobotFullPrompt, copyRobotPrompt } from "@/lib/robot-prompt-copy"
import { showRobotEventToast } from "@/lib/robot-toast"
import type { RunningMove } from "@/lib/running-move"
import { robotRestartMove } from "@/lib/running-move"
import { showToast } from "@/lib/toast-message"
import { stackReadinessKey } from "@/lib/whole-stack"
import { startCheck } from "@/presenters/reasons"
import { recordRobot } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import type { OperationKey, Operations } from "@/stores/operations"
import { getOperations, operationItem, runOperation } from "@/stores/operations"
import { cancelPatronusRobot, getRobotRestart, isRobotFailing, isRobotNeedingReview, restartPatronusRobot } from "@/stores/patronus"
import { getSelectedRepositoryPath } from "@/stores/repository"

/**
 * Says that a robot started, restarted or is canceling, in a toast and in Activity: "Safe Push started" / "auth → master
 * · profile Large Scale Change", with "Open in Patronus" (a toast about an object opens it)
 */
export function announceRobot(title: string, robot: PatronusRobot, profileName: string | null = null): void {
  showRobotToast(title, robot, profileName)
  recordRobot(title, describeRobot(robot, profileName), robot)
}

/**
 * The toast of `announceRobot` alone, for a robot Activity has already (a restart after a push): the toast of the robot
 * in the repository the main window shows, where its action ran, which takes the place of the toast of the robot it
 * restarted or cancels ("Safe Push of auth failed" becomes "Safe Push restarted")
 */
export function showRobotToast(title: string, robot: PatronusRobot, profileName: string | null = null): void {
  showRobotEventToast({ repositoryPath: getSelectedRepositoryPath(), robot, type: "success", title, description: describeRobot(robot, profileName) })
}

/**
 * The operation of a restart (`restart-robot`, `push-and-restart-robot`, which pushes first) or a cancel (`cancel-robot`)
 * of the robot: one at a time per robot
 */
export function robotOperation(id: "restart-robot" | "push-and-restart-robot" | "cancel-robot", robot: PatronusRobot): OperationKey {
  return { id, subject: `robot:${robot.id}`, objects: [], kind: "remote" }
}

/** A restart is the move of the row of the robot (its branch, or the whole stack): its node lands from the click on */
function restartMoveOf(robot: PatronusRobot): { move: RunningMove } | null {
  const key = robot.branchName ?? (robot.stackRoot == null ? null : stackReadinessKey(robot.stackRoot))
  return key === null ? null : { move: robotRestartMove(key, robot.pushMode) }
}

/**
 * Runs a restart or a cancel of a robot from a menu, a popover or a toast as one operation of the robot
 * (`runOperation`: a second click joins it, the other one waits), and says how it went
 */
export async function runRobotAction(id: "restart-robot" | "cancel-robot", robot: PatronusRobot): Promise<void> {
  const mode = formatPushMode(robot.pushMode)
  const restart = id === "restart-robot"
  const failure = restart ? `Cannot restart the ${mode}` : `Cannot cancel the ${mode}`
  await runOperation(
    { ...robotOperation(id, robot), label: restart ? `Restarting the ${mode}` : `Canceling the ${mode}`, ...(restart ? restartMoveOf(robot) : null) },
    () =>
      (restart ? restartPatronusRobot(robot) : cancelPatronusRobot(robot)).then(
        result => announceRobot(restart ? `${mode} restarted` : `${mode} is canceling`, result),
        (error: unknown) => notifyError(failure, error),
      ),
    reason => showToast({ type: "info", title: failure, description: reason }),
  )
}

/**
 * Pushes the branch (`branchName`) or the stack branch of the whole stack (`root`) and restarts the robot on the pushed
 * commit, as one operation of the robot: a second click joins it, its cancel waits, and the row lands from the click
 * on, the push included. The restart takes the commit just pushed, and its toast is the one of the action.
 */
function runPushAndRestart(robot: PatronusRobot, branchName: string | null, root: string | null): Promise<unknown> {
  const mode = formatPushMode(robot.pushMode)
  return runOperation(
    { ...robotOperation("push-and-restart-robot", robot), label: `Pushing and restarting the ${mode}`, ...restartMoveOf(robot) },
    () => (root === null ? pushBranch(branchName!, { restartRobot: robot, announce: false }) : pushWholeStack(root, { restartRobot: robot })),
    reason => showToast({ type: "info", title: `Cannot restart the ${mode}`, description: reason }),
  )
}

/** An action of `getRobotActions`, named by its `id` */
export interface RobotAction extends MenuItem {
  id: "restart-robot" | "push-and-restart-robot" | "cancel-robot" | "copy-robot-prompt" | "copy-robot-full-prompt" | "open-robot" | "open-robot-pull-request"
}

/** A robot whose failure an agent can fix: it failed (not for missing approvals), or a required check failed while it runs */
export function hasRobotFailurePrompt(robot: PatronusRobot, needsReview: boolean, failing: boolean): boolean {
  return robot.status === "failure" ? !needsReview : failing && robot.status === "running"
}

/**
 * The actions of a robot, one list for every surface that shows it (the branch menu, the robot details, the robots
 * indicator), in this order: "Restart Safe Push" (it pushes first while the branch has work the remote lacks, also
 * for a running robot, which it cancels first; disabled with the reason while a start is not possible: a blocked
 * branch, a robot that lands an ancestor or the branch, a branch a merged Safe Push landed whole: the `restarts` of the
 * view, `domain_policy::holds::restart_action` and `land::restart_refusal`), "Cancel Safe Push" while it runs,
 * "Copy Prompt for Agent" and "Copy Prompt with CI Details" for a failed or failing robot (`hasRobotFailurePrompt`),
 * "Open in Patronus", and "Open Pull Request #12" for a robot of a pull request. `branch` is the branch of the robot,
 * null when it has no row (archived since it merged): the restart then takes the remote branch as Patronus has it.
 * The robot of the whole stack (`stackRoot`) has no branch: its restart works on the stack branch, which a push of it
 * sends first (`pushWholeStack`), waits for a robot of a branch of the stack, and is refused once every branch of the
 * stack landed.
 * `needsReview`: the robot failed for missing approvals; a component passes the value it subscribed to, so its list
 * follows it; `failing` likewise: a required check of the running robot failed.
 */
export function getRobotActions(
  robot: PatronusRobot,
  branch: Branch | null,
  needsReview = isRobotNeedingReview(robot.id),
  failing = isRobotFailing(robot.id),
  operations: Operations = getOperations(),
): RobotAction[] {
  const mode = formatPushMode(robot.pushMode)
  const items: RobotAction[] = []
  // what its restart does and why it can't start now, as Rust decides it of the view (`restart_action`,
  // `restart_refusal`: a restart is a start, it waits for a robot that lands an ancestor or the branch itself)
  const restartOf = getRobotRestart(robot.id)
  const restart = restartOf?.action ?? null
  const root = robot.branchName === null ? (robot.stackRoot ?? null) : null
  const start = restartOf === null ? null : startCheck(restartOf.refusal)
  if (restart === "restart") {
    items.push(
      operationItem(
        operations,
        {
          id: "restart-robot",
          label: formatRobotRestartLabel(robot),
          icon: RotateCcwIcon,
          ...(start?.ok === false && { disabled: true, description: start.reason }),
          onSelect: () => void runRobotAction("restart-robot", robot),
        },
        robotOperation("restart-robot", robot),
      ) as RobotAction,
    )
  } else if (restart === "pushAndRestart" && (branch !== null || root !== null)) {
    // a running robot tests a commit the branch moved past: the push cancels it first, the one push into its branch; the
    // push is part of the restart, its name is the restart's
    const running = robot.status === "running" && robot.sourceRevision !== null
    const pushed = root === null ? branch!.name : formatRobotSubject(robot)
    items.push(
      operationItem(
        operations,
        {
          id: "push-and-restart-robot",
          label: formatRobotRestartLabel(robot),
          icon: RotateCcwIcon,
          ...(start?.ok === false
            ? { disabled: true, description: start.reason }
            : {
                description: running
                  ? `Cancels the run on ${formatShortHash(robot.sourceRevision!)}, pushes ${pushed} and starts it again on the pushed commit`
                  : `Pushes ${pushed} and starts it again on the pushed commit`,
              }),
          onSelect: () => void runPushAndRestart(robot, root === null ? branch!.name : null, root),
        },
        robotOperation("push-and-restart-robot", robot),
      ) as RobotAction,
    )
  }
  if (canCancelRobot(robot)) {
    items.push(
      operationItem(
        operations,
        { id: "cancel-robot", label: `Cancel ${mode}`, icon: ShieldOffIcon, onSelect: () => void runRobotAction("cancel-robot", robot) },
        robotOperation("cancel-robot", robot),
      ) as RobotAction,
    )
  }
  if (hasRobotFailurePrompt(robot, needsReview, failing)) {
    items.push(
      { id: "copy-robot-prompt", label: "Copy Prompt for Agent", icon: ClipboardCopyIcon, onSelect: () => void copyRobotPrompt(robot) },
      { id: "copy-robot-full-prompt", label: "Copy Prompt with CI Details", icon: ClipboardListIcon, onSelect: () => void copyRobotFullPrompt(robot) },
    )
  }
  items.push({ id: "open-robot", label: "Open in Patronus", icon: ExternalLinkIcon, onSelect: () => openRobot(robot) })
  const pullRequest = robot.pullRequest
  if (pullRequest != null) {
    items.push({ id: "open-robot-pull-request", label: `Open Pull Request #${pullRequest.number}`, icon: GitPullRequestIcon, onSelect: () => openPullRequestUrl(pullRequest.url) })
  }
  return items
}
