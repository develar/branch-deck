import { toast } from "@/components/ui/toast"
import type { PatronusMergeRequest, PatronusRobot } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { cancelHoldingRobot, pushForAction } from "@/lib/git/push"
import type { PullRequestStart } from "@/lib/patronus"
import { canCancelRobot, describeRobot, formatPushMode, openRobot } from "@/lib/patronus"
import { runRobotAction } from "@/lib/patronus-actions"
import { ACTION_TOAST_TIMEOUT_MS } from "@/lib/toast-message"
import { recordRobot } from "@/stores/activity"
import { cancelPatronusRobot, startPatronusFromPullRequest, startPatronusRobot } from "@/stores/patronus"
import type { SafePushMode } from "@/stores/safe-push"
import { getSafePushStarting, setSafePushStarting } from "@/stores/safe-push"

/** What a start runs, as `getSafePushDecision` let it */
export interface SafePushStart {
  readonly branchName: string
  readonly mode: SafePushMode
  /** The merge profile; null: the default checks. A comment on a pull request can't choose one */
  readonly profile: { readonly id: string; readonly name: string } | null
  /** The open pull request into the target the start comments on; null: through the API */
  readonly fromPullRequest: PullRequestStart | null
  /** The open merge request of a repository only on Space, whose approvals count for the start */
  readonly mergeRequest: PatronusMergeRequest | null
  /** The branch, or the stack it is built on, has work the remote lacks: pushed first */
  readonly needsPush: boolean
  /** A Safe Push of the branch on an older commit: canceled for the start (for its push, when it pushes) */
  readonly replaces: PatronusRobot | null
}

/**
 * Starts a Safe Push or a Dry Run: the push of what the remote lacks first (`pushForAction`: the stack root first, a
 * running Safe Push of an older commit canceled for it), then the robot, from the pull request (a `/patronus` comment)
 * or through the API with the profile. Its steps show in the button of the slot (`SafePushStarting`); the one toast of
 * the action is the start's ("Safe Push started", with Open in Patronus and Cancel Safe Push), the push has none
 * (Activity records it). A failure is "Cannot start the Safe Push" with the reason; a failed push starts nothing.
 * Resolves to whether the robot started.
 */
export async function startSafePush(start: SafePushStart): Promise<boolean> {
  const { branchName, mode } = start
  const modeName = formatPushMode(mode)
  const current = getSafePushStarting(branchName)
  if (current !== null && current.step !== "checking") {
    return false
  }
  const errorTitle = `Cannot start the ${modeName}`
  try {
    if (start.needsPush) {
      setSafePushStarting(branchName, { mode, step: "pushing" })
      const pushed = await pushForAction(branchName, { replacesRobot: start.replaces ?? undefined })
      if (!pushed.pushed) {
        notifyError(errorTitle, pushed.reason)
        return false
      }
    } else if (start.replaces !== null && !(await cancelHoldingRobot(start.replaces))) {
      return false
    }
    setSafePushStarting(branchName, { mode, step: "starting" })
    const robot =
      start.fromPullRequest === null
        ? await startPatronusRobot(branchName, mode, start.profile?.id ?? null, start.mergeRequest, start.profile?.name ?? null)
        : await startPatronusFromPullRequest(start.fromPullRequest.number, mode)
    announceStart(`${modeName} started`, robot, start.fromPullRequest === null ? (start.profile?.name ?? null) : null)
    return true
  } catch (error) {
    notifyError(errorTitle, error)
    return false
  } finally {
    setSafePushStarting(branchName, null)
  }
}

/**
 * The toast of a started robot, as `announceRobot` says it, with "Cancel Safe Push" below the text: a start asked by
 * mistake costs only the robot (the push and the comment stay); Activity records it
 */
function announceStart(title: string, robot: PatronusRobot, profileName: string | null): void {
  const description = describeRobot(robot, profileName)
  recordRobot(title, description, robot)
  const mode = formatPushMode(robot.pushMode)
  const id = toast.add({
    type: "success",
    title,
    description,
    timeout: ACTION_TOAST_TIMEOUT_MS,
    actionProps: { children: "Open in Patronus", onClick: () => openRobot(robot) },
    ...(canCancelRobot(robot) && {
      data: {
        secondaryAction: {
          label: `Cancel ${mode}`,
          onClick: () => {
            toast.close(id)
            void runRobotAction(cancelPatronusRobot, robot, `${mode} is canceling`, `Cannot cancel the ${mode}`)
          },
        },
      },
    }),
  })
}
