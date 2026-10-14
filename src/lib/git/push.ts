import { toast } from "@/components/ui/toast"
import type { PatronusRobot } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { showStackedPushToast } from "@/lib/git/stacked-push-toast"
import { formatPushMode, isPushIntoRobotRisky } from "@/lib/patronus"
import { runRobotAction } from "@/lib/patronus-actions"
import { stackPushOrder } from "@/lib/stack"
import { branchSyncStore, getBranch, isBranchPushing, setBranchPushing, setBranchRemoteStatus, toRemoteStatus } from "@/stores/branch-sync"
import { cancelPatronusRobot, fetchPatronusRobotStatus, getBranchRobot, restartPatronusRobot } from "@/stores/patronus"
import { refreshPullRequestsAfterPush } from "@/stores/pull-requests"
import type { PushGuardAnswer } from "@/stores/push-guard"
import { askPushGuard } from "@/stores/push-guard"
import { vcsRequestFactory } from "@/stores/repository"

/**
 * Pushes a virtual branch and updates its remote status from the result, then the pull requests (the pull request of
 * the branch now shows the pushed commit). A push into a branch whose Safe Push runs asks first (see
 * `guardRunningRobot`). Errors are reported with a toast. Resolves to whether the push succeeded.
 */
export async function pushBranch(branchName: string): Promise<boolean> {
  if (isBranchPushing(branchName)) {
    return false
  }

  const branch = getBranch(branchName)
  if (branch === undefined) {
    notifyError("Push Failed", `Branch ${branchName} not found`)
    return false
  }

  const guard = await guardRunningRobot(branchName)
  if (guard.answer === "back") {
    return false
  }
  if (guard.answer === "cancel-push-restart") {
    try {
      await cancelPatronusRobot(guard.robot)
    }
    catch (error) {
      notifyError("Cannot cancel the Safe Push", error)
      return false
    }
  }

  setBranchPushing(branchName, true)
  try {
    const request = vcsRequestFactory.createRequest()
    const result = await commands.pushBranch({
      repositoryPath: request.repositoryPath,
      branchPrefix: request.branchPrefix,
      branchName,
      myEmail: branch.myEmail,
      baselineBranch: branchSyncStore.getState().baselineBranch!,
      // a stacked branch's own commits are those after its parent
      parentBranch: branch.stack?.effectiveBase ?? null,
    })

    if (result.status === "ok") {
      if (branch.stack === null) {
        toast.add({
          type: "success",
          title: "Success",
          description: `Branch ${branchName} pushed successfully`,
        })
      }
      else {
        showStackedPushToast(branch)
      }
      setBranchRemoteStatus(branchName, toRemoteStatus(result.data))
      void refreshPullRequestsAfterPush(branchName, branch.commits[0]?.hash)
      if (guard.answer === "cancel-push-restart") {
        // the restart takes the commit just pushed
        void runRobotAction(restartPatronusRobot, guard.robot, `${formatPushMode(guard.robot.pushMode)} restarted`, `Cannot restart the ${formatPushMode(guard.robot.pushMode)}`)
      }
      return true
    }
    notifyError("Push Failed", result.error)
  }
  catch (error) {
    notifyError("Push Failed", error)
  }
  finally {
    setBranchPushing(branchName, false)
  }
  return false
}

type RobotGuard = { answer: "push" } | { answer: PushGuardAnswer, robot: PatronusRobot }

/**
 * A push into a branch whose Safe Push runs makes the robot fail at the end ("branch was modified"): the user chooses
 * to cancel it, push and restart it on the pushed commit, to push anyway, or to go back. The status is read fresh
 * first (one row on the server), so a robot that finished meanwhile asks nothing.
 */
async function guardRunningRobot(branchName: string): Promise<RobotGuard> {
  const robot = getBranchRobot(branchName)
  if (robot === undefined || !isPushIntoRobotRisky(robot, branchName)) {
    return { answer: "push" }
  }
  const status = await fetchPatronusRobotStatus(robot).catch(() => null)
  if (status !== null && status !== "running") {
    return { answer: "push" }
  }
  return { answer: await askPushGuard(branchName, robot), robot }
}

/** Pushes the ancestors of a stacked branch that have work for the remote, root first, then the branch; stops at a failure */
export async function pushStack(branchName: string): Promise<void> {
  for (const name of stackPushOrder(branchName, getBranch)) {
    if (!await pushBranch(name)) {
      return
    }
  }
}
