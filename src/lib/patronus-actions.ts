import { ClipboardCopyIcon, ClipboardListIcon, ExternalLinkIcon, GitPullRequestIcon, RotateCcwIcon, ShieldOffIcon } from "lucide-react"
import type { PatronusRobot } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { pushBranch } from "@/lib/git/push"
import { pushWholeStack } from "@/lib/git/stack-push"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem } from "@/lib/menu"
import type { RobotStartCheck, RobotSubject } from "@/lib/patronus"
import {
  canCancelRobot,
  canStartRobot,
  canStartStackRobot,
  describeRobot,
  formatLandedReason,
  formatPushMode,
  formatRobotRestartLabel,
  formatRobotSubject,
  getRobotRestartAction,
  isLandedWhole,
  openRobot,
} from "@/lib/patronus"
import { openPullRequestUrl } from "@/lib/pull-request"
import { copyRobotFullPrompt, copyRobotPrompt } from "@/lib/robot-prompt-copy"
import { showToast } from "@/lib/toast-message"
import { getStackBranchBlocker } from "@/lib/whole-stack"
import { recordRobot } from "@/stores/activity"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import { getStackBranch } from "@/stores/branch-sync"
import type { OperationKey, Operations } from "@/stores/operations"
import { getOperations, operationItem, runOperation } from "@/stores/operations"
import {
  cancelPatronusRobot,
  getLandedBy,
  getLandingAncestor,
  getLandingSpan,
  getStackLandedReason,
  getStackLandingMember,
  isRobotFailing,
  isRobotNeedingReview,
  restartPatronusRobot,
} from "@/stores/patronus"

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

/** The operation of a restart (`restart-robot`) or a cancel (`cancel-robot`) of the robot: one at a time per robot */
export function robotOperation(id: "restart-robot" | "cancel-robot", robot: PatronusRobot): OperationKey {
  return { id, subject: `robot:${robot.id}`, objects: [], kind: "remote" }
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
    { ...robotOperation(id, robot), label: restart ? `Restarting the ${mode}` : `Canceling the ${mode}` },
    () =>
      (restart ? restartPatronusRobot(robot) : cancelPatronusRobot(robot)).then(
        result => announceRobot(restart ? `${mode} restarted` : `${mode} is canceling`, result),
        (error: unknown) => notifyError(failure, error),
      ),
    reason => showToast({ type: "info", title: failure, description: reason }),
  )
}

/**
 * Whether the robot of `branch` can restart: not for a blocked branch, nor, for a Safe Push, while a robot lands an
 * ancestor of the branch or a branch built on it lands the branch (`canStartRobot`): two robots would land the same
 * commits. A dry run pushes nothing into the target, so only the block holds it back. Nor once a merged Safe Push
 * landed all of the branch, a Dry Run too: its commits are in the target, as the next step says (row 1, "Landed in
 * master with oauth").
 */
function checkRobotRestart(robot: PatronusRobot, branch: Branch): RobotStartCheck {
  const landing = robot.pushMode !== "dryRun"
  const check = canStartRobot(branch, undefined, landing ? getLandingAncestor(branch.name) : null, landing ? getLandingSpan(branch.name) : null)
  const landed = getLandedBy(branch.name)
  return check.ok && isLandedWhole(landed) ? { ok: false, reason: formatLandedReason(landed) } : check
}

/**
 * Whether the robot of the whole stack of `root` can restart: on a stack branch the sync built, and, for a Safe Push,
 * not while the robot of a branch of the stack lands a part of it (`canStartStackRobot`). Nor once merged Safe Pushes
 * landed every branch of the stack, a Dry Run too: its stack branch carries only commits that are in the target
 * (`getStackLandedReason`, "Landed in master with deploy and side-test"), as its Safe Push Whole Stack says.
 */
function checkStackRobotRestart(robot: PatronusRobot, root: string): RobotStartCheck {
  const stack = getStackBranch(root)
  const landing = robot.pushMode !== "dryRun"
  const check = canStartStackRobot(
    getStackBranchBlocker(root, stack),
    undefined,
    stack?.tip ?? undefined,
    stack?.remoteStatus ?? null,
    landing ? getStackLandingMember(root) : null,
    !landing,
  )
  const landed = check.ok ? getStackLandedReason(root) : null
  return landed === null ? check : { ok: false, reason: landed }
}

/** What a restart of the robot works on: the branch, or the stack branch of the whole stack */
interface RestartTarget {
  readonly on: string | RobotSubject
  readonly tip: string | undefined
  readonly remote: RemoteStatus | null
}

function restartTargetOf(robot: PatronusRobot, branch: Branch | null): RestartTarget | null {
  const root = robot.branchName === null ? (robot.stackRoot ?? null) : null
  if (root !== null) {
    const stack = getStackBranch(root)
    return { on: { kind: "stack", root }, tip: stack?.tip ?? undefined, remote: stack?.remoteStatus ?? null }
  }
  const branchName = branch?.name ?? robot.branchName
  return branchName === null ? null : { on: branchName, tip: branch?.commits[0]?.hash, remote: branch?.remoteStatus ?? null }
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
 * for a running robot, which it cancels first, `getRobotRestartAction`; disabled with the reason while a start is not
 * possible: a blocked branch, a robot that lands an ancestor or the branch, a branch a merged Safe Push landed whole,
 * `checkRobotRestart`), "Cancel Safe Push"
 * while it runs,
 * "Copy Prompt for Agent" and "Copy Prompt with CI Details" for a failed or failing robot (`hasRobotFailurePrompt`),
 * "Open in Patronus", and "Open Pull Request #12" for a robot of a pull request. `branch` is the branch of the robot,
 * null when it has no row (archived since it merged): the restart then takes the remote branch as Patronus has it.
 * The robot of the whole stack (`stackRoot`) has no branch: its restart works on the stack branch, which a push of it
 * sends first (`pushWholeStack`), waits for a robot of a branch of the stack, and is refused once every branch of the
 * stack landed (`checkStackRobotRestart`).
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
  const target = restartTargetOf(robot, branch)
  const restart = target === null ? null : getRobotRestartAction(robot, target.on, target.tip, target.remote, needsReview)
  const root = typeof target?.on === "object" && target.on.kind === "stack" ? target.on.root : null
  // a restart is a start: it waits for a robot that lands an ancestor or the branch itself, as "Safe Push" does
  let start: RobotStartCheck | null = null
  if (restart !== null) {
    start = root !== null ? checkStackRobotRestart(robot, root) : branch === null ? null : checkRobotRestart(robot, branch)
  }
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
    items.push({
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
      // the restart takes the commit just pushed, and its toast is the one of the action
      onSelect: () => void (root === null ? pushBranch(branch!.name, { restartRobot: robot, announce: false }) : pushWholeStack(root, { restartRobot: robot })),
    })
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
