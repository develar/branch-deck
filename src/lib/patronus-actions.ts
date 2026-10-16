import { ExternalLinkIcon, GitPullRequestIcon, RotateCcwIcon, ShieldOffIcon, UploadIcon } from "lucide-react"
import type { PatronusRobot } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { pushBranch } from "@/lib/git/push"
import type { MenuItem } from "@/lib/menu"
import { canCancelRobot, describeRobot, formatPushMode, formatRobotRestartLabel, getRobotRestartAction, openRobot } from "@/lib/patronus"
import { openPullRequestUrl } from "@/lib/pull-request"
import { formatBlockedReason } from "@/lib/stack"
import { showToast } from "@/lib/toast-message"
import { recordRobot } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import { cancelPatronusRobot, isRobotNeedingReview, restartPatronusRobot } from "@/stores/patronus"

/**
 * Says that a robot started, restarted or is canceling, in a toast and in Activity: "Safe Push started" / "auth → master
 * · profile Large Scale Change", with "Open in Patronus" (a toast about an object opens it)
 */
export function announceRobot(title: string, robot: PatronusRobot, profileName: string | null = null): void {
  showRobotToast(title, robot, profileName)
  recordRobot(title, describeRobot(robot, profileName), robot)
}

/** The toast of `announceRobot` alone, for a robot Activity has already (a restart after a push) */
export function showRobotToast(title: string, robot: PatronusRobot, profileName: string | null = null): void {
  showToast({ title, description: describeRobot(robot, profileName), action: { children: "Open in Patronus", onClick: () => openRobot(robot) } })
}

/** Runs a restart or a cancel of a robot from a menu or a popover, and says how it went */
export async function runRobotAction(action: (robot: PatronusRobot) => Promise<PatronusRobot>, robot: PatronusRobot, success: string, failure: string): Promise<void> {
  let result: PatronusRobot
  try {
    result = await action(robot)
  }
  catch (error) {
    notifyError(failure, error)
    return
  }
  announceRobot(success, result)
}

/** An action of `getRobotActions`, named by its `id` */
export interface RobotAction extends MenuItem {
  id: "restart-robot" | "push-and-restart-robot" | "cancel-robot" | "open-robot" | "open-robot-pull-request"
}

/**
 * The actions of a robot, one list for every surface that shows it (the branch menu, the robot details, the robots
 * indicator), in this order: "Restart Safe Push", or "Push and Restart Safe Push" while the branch has work the remote
 * lacks (`getRobotRestartAction`; disabled for a blocked branch, with the reason), "Cancel Safe Push" while it runs,
 * "Open in Patronus", and "Open Pull Request #12" for a robot of a pull request. `branch` is the branch of the robot,
 * null when it has no row (archived since it merged): the restart then takes the remote branch as Patronus has it.
 * `needsReview`: the robot failed for missing approvals; a component passes the value it subscribed to, so its list
 * follows it.
 */
export function getRobotActions(robot: PatronusRobot, branch: Branch | null, needsReview = isRobotNeedingReview(robot.id)): RobotAction[] {
  const mode = formatPushMode(robot.pushMode)
  const items: RobotAction[] = []
  const branchName = branch?.name ?? robot.branchName
  const restart = branchName === null ? null : getRobotRestartAction(robot, branchName, branch?.commits[0]?.hash, branch?.remoteStatus ?? null, needsReview)
  if (restart === "restart") {
    items.push({
      id: "restart-robot",
      label: formatRobotRestartLabel(restart, robot),
      icon: RotateCcwIcon,
      onSelect: () => void runRobotAction(restartPatronusRobot, robot, `${mode} restarted`, `Cannot restart the ${mode}`),
    })
  }
  else if (restart === "pushAndRestart" && branch !== null) {
    const blocked = branch.status === "Blocked"
    items.push({
      id: "push-and-restart-robot",
      label: formatRobotRestartLabel(restart, robot),
      icon: UploadIcon,
      disabled: blocked,
      ...(blocked && { description: formatBlockedReason(branch) }),
      // the restart takes the commit just pushed, and its toast is the one of the action
      onSelect: () => void pushBranch(branch.name, { restartRobot: robot, announce: false }),
    })
  }
  if (canCancelRobot(robot)) {
    items.push({
      id: "cancel-robot",
      label: `Cancel ${mode}`,
      icon: ShieldOffIcon,
      onSelect: () => void runRobotAction(cancelPatronusRobot, robot, `${mode} is canceling`, `Cannot cancel the ${mode}`),
    })
  }
  items.push({ id: "open-robot", label: "Open in Patronus", icon: ExternalLinkIcon, onSelect: () => openRobot(robot) })
  const pullRequest = robot.pullRequest
  if (pullRequest != null) {
    items.push({ id: "open-robot-pull-request", label: `Open Pull Request #${pullRequest.number}`, icon: GitPullRequestIcon, onSelect: () => openPullRequestUrl(pullRequest.url) })
  }
  return items
}
