import { ClipboardCopyIcon, ClipboardListIcon, ExternalLinkIcon, GitPullRequestIcon, RotateCcwIcon, ShieldOffIcon, UploadIcon } from "lucide-react"
import type { PatronusRobot } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { pushBranch } from "@/lib/git/push"
import type { MenuItem } from "@/lib/menu"
import type { RobotStartCheck } from "@/lib/patronus"
import { canCancelRobot, canStartRobot, describeRobot, formatPushMode, formatRobotRestartLabel, getRobotRestartAction, openRobot } from "@/lib/patronus"
import { openPullRequestUrl } from "@/lib/pull-request"
import { copyRobotFullPrompt, copyRobotPrompt } from "@/lib/robot-prompt-copy"
import { showToast } from "@/lib/toast-message"
import { recordRobot } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import { cancelPatronusRobot, getLandingAncestor, getLandingSpan, isRobotFailing, isRobotNeedingReview, restartPatronusRobot } from "@/stores/patronus"

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

/**
 * Whether the robot of `branch` can restart: not for a blocked branch, nor, for a Safe Push, while a robot lands an
 * ancestor of the branch or a branch built on it lands the branch (`canStartRobot`): two robots would land the same
 * commits. A dry run pushes nothing into the target, so only the block holds it back.
 */
function checkRobotRestart(robot: PatronusRobot, branch: Branch): RobotStartCheck {
  const landing = robot.pushMode !== "dryRun"
  return canStartRobot(branch, undefined, landing ? getLandingAncestor(branch.name) : null, landing ? getLandingSpan(branch.name) : null)
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
 * indicator), in this order: "Restart Safe Push", or "Push and Restart Safe Push" while the branch has work the remote
 * lacks (`getRobotRestartAction`; disabled with the reason while a start is not possible: a blocked branch, a robot that
 * lands an ancestor or the branch, `checkRobotRestart`), "Cancel Safe Push" while it runs,
 * "Copy Prompt for Agent" and "Copy Prompt with CI Details" for a failed or failing robot (`hasRobotFailurePrompt`),
 * "Open in Patronus", and "Open Pull Request #12" for a robot of a pull request. `branch` is the branch of the robot,
 * null when it has no row (archived since it merged): the restart then takes the remote branch as Patronus has it.
 * `needsReview`: the robot failed for missing approvals; a component passes the value it subscribed to, so its list
 * follows it; `failing` likewise: a required check of the running robot failed.
 */
export function getRobotActions(robot: PatronusRobot, branch: Branch | null, needsReview = isRobotNeedingReview(robot.id), failing = isRobotFailing(robot.id)): RobotAction[] {
  const mode = formatPushMode(robot.pushMode)
  const items: RobotAction[] = []
  const branchName = branch?.name ?? robot.branchName
  const restart = branchName === null ? null : getRobotRestartAction(robot, branchName, branch?.commits[0]?.hash, branch?.remoteStatus ?? null, needsReview)
  // a restart is a start: it waits for a robot that lands an ancestor or the branch itself, as "Safe Push…" does
  const start = branch === null || restart === null ? null : checkRobotRestart(robot, branch)
  if (restart === "restart") {
    items.push({
      id: "restart-robot",
      label: formatRobotRestartLabel(restart, robot),
      icon: RotateCcwIcon,
      ...(start?.ok === false && { disabled: true, description: start.reason }),
      onSelect: () => void runRobotAction(restartPatronusRobot, robot, `${mode} restarted`, `Cannot restart the ${mode}`),
    })
  }
  else if (restart === "pushAndRestart" && branch !== null) {
    items.push({
      id: "push-and-restart-robot",
      label: formatRobotRestartLabel(restart, robot),
      icon: UploadIcon,
      ...(start?.ok === false && { disabled: true, description: start.reason }),
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
