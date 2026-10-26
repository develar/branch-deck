import type { ActionState, PatronusRobot } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { pushBranch } from "@/lib/git/push"
import { pushWholeStack } from "@/lib/git/stack-push"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem } from "@/lib/menu"
import { describeRobot, formatPushMode, formatRobotRestartLabel, formatRobotSubject, openRobot } from "@/lib/patronus"
import { openPullRequestUrl } from "@/lib/pull-request"
import { copyRobotFullPrompt, copyRobotPrompt } from "@/lib/robot-prompt-copy"
import { showRobotEventToast } from "@/lib/robot-toast"
import { showToast } from "@/lib/toast-message"
import { stackReadinessKey } from "@/lib/whole-stack"
import { getRowPart } from "@/model/store"
import type { ActionItem } from "@/presenters/actions"
import { presentAction } from "@/presenters/actions"
import type { RunningMove } from "@/presenters/running-move"
import { robotRestartMove } from "@/presenters/running-move"
import { recordRobot } from "@/stores/activity"
import type { OperationKey, Operations } from "@/stores/operations"
import { getOperations, operationItem, runOperation } from "@/stores/operations"
import { cancelPatronusRobot, restartPatronusRobot } from "@/stores/patronus"
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

/** An action of a robot (`getRobotActionItems`), named by its `id` */
export interface RobotAction extends MenuItem {
  id: "restart-robot" | "push-and-restart-robot" | "cancel-robot" | "copy-robot-prompt" | "copy-robot-full-prompt" | "open-robot" | "open-robot-pull-request"
}

/** What each action of a robot runs and how the facts of the robot name it */
function robotActionItem(robot: PatronusRobot, id: RobotAction["id"]): ActionItem {
  const mode = formatPushMode(robot.pushMode)
  switch (id) {
    case "restart-robot":
      return { label: formatRobotRestartLabel(robot), onSelect: () => void runRobotAction("restart-robot", robot) }
    case "push-and-restart-robot": {
      // a running robot tests a commit the branch moved past: the push cancels it first, the one push into its branch;
      // the push is part of the restart, its name is the restart's
      const root = robot.branchName === null ? (robot.stackRoot ?? null) : null
      const pushed = root === null ? (robot.branchName ?? "") : formatRobotSubject(robot)
      const runs = robot.status === "running" && robot.sourceRevision !== null
      return {
        label: formatRobotRestartLabel(robot),
        description: runs
          ? `Cancels the run on ${formatShortHash(robot.sourceRevision!)}, pushes ${pushed} and starts it again on the pushed commit`
          : `Pushes ${pushed} and starts it again on the pushed commit`,
        onSelect: () => void runPushAndRestart(robot, root === null ? robot.branchName : null, root),
      }
    }
    case "cancel-robot":
      return { label: `Cancel ${mode}`, onSelect: () => void runRobotAction("cancel-robot", robot) }
    case "copy-robot-prompt":
      return { onSelect: () => void copyRobotPrompt(robot) }
    case "copy-robot-full-prompt":
      return { onSelect: () => void copyRobotFullPrompt(robot) }
    case "open-robot":
      return { onSelect: () => openRobot(robot) }
    case "open-robot-pull-request": {
      const pullRequest = robot.pullRequest
      return { label: `Open Pull Request #${pullRequest?.number ?? ""}`, kbds: null, onSelect: () => pullRequest != null && openPullRequestUrl(pullRequest.url) }
    }
  }
}

/** The operation of an action of a robot that one runs at a time per robot: a restart, a push and restart, a cancel */
function robotOperationOf(robot: PatronusRobot, id: RobotAction["id"]): OperationKey | null {
  return id === "restart-robot" || id === "push-and-restart-robot" || id === "cancel-robot" ? robotOperation(id, robot) : null
}

/**
 * The actions of a robot as Rust offers them (`robot:<id>` of `RepositoryView.actions`, `domain_policy::actions`), one
 * list for every surface that shows it (the branch menu, the robot details, the robots indicator, a toast), in this
 * order: "Restart Safe Push" (it pushes first while the branch has work the remote lacks, also for a running robot,
 * which it cancels first), "Cancel Safe Push" while it runs, "Copy Prompt for Agent" and "Copy Prompt with CI Details"
 * for a failed or failing robot, "Open in Patronus", and "Open Pull Request #12" for a robot of a pull request. Rust
 * disables a restart and the prompts while Patronus didn't tell why the robot failed, a restart while the approvals the
 * step of its branch misses (`stepReason`: the words of that step, `BranchNextStep.safePushBlocker`) or what refuses a
 * start stands in its way. A restart, a push and restart and a cancel run one at a time per robot (`operationItem`).
 * `offered`: the actions of the robot a component subscribed to, so its list follows them.
 */
export function getRobotActionItems(
  robot: PatronusRobot,
  operations: Operations = getOperations(),
  stepReason: string | null = null,
  offered: readonly ActionState[] | undefined = getRowPart(`robot:${robot.id}`, "actions"),
): RobotAction[] {
  const actions = offered ?? []
  const words = { subject: robot.branchName ?? robot.stackRoot ?? "", step: stepReason }
  return actions.flatMap(action => {
    const id = action.id as RobotAction["id"]
    const item = presentAction(actions, id, robotActionItem(robot, id), words)
    if (item === null) {
      return []
    }
    const key = robotOperationOf(robot, id)
    return [(key === null ? item : operationItem(operations, item, key)) as RobotAction]
  })
}

/** The restart of a robot, which pushes first or not */
export function isRestart(item: RobotAction): boolean {
  return item.id === "restart-robot" || item.id === "push-and-restart-robot"
}
