import { Channel } from "@tauri-apps/api/core"
import { toast } from "@/components/ui/toast"
import type { PatronusRobot, TransferProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { getPullRequestToastAction } from "@/lib/git/new-pull-request"
import { showStackedPushToast } from "@/lib/git/stacked-push-toast"
import { formatPushMode, isPushIntoRobotRisky } from "@/lib/patronus"
import { runRobotAction } from "@/lib/patronus-actions"
import { describePush, dropsRemoteCommits } from "@/lib/push-summary"
import { buildRemoteMovedTexts } from "@/lib/remote-status"
import { stackPushOrder } from "@/lib/stack"
import { showToast } from "@/lib/toast-message"
import { recordPush } from "@/stores/activity"
import type { RemoteStatus } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, isBranchPushing, setBranchPushing, setBranchPushProgress, setBranchRemoteStatus, toRemoteStatus } from "@/stores/branch-sync"
import { cancelPatronusRobot, fetchPatronusRobotStatus, getBranchRobot, restartPatronusRobot } from "@/stores/patronus"
import { refreshPullRequestsAfterPush } from "@/stores/pull-requests"
import type { PushConfirmTarget } from "@/stores/push-confirm"
import { requestPushConfirm } from "@/stores/push-confirm"
import type { PushGuardAnswer } from "@/stores/push-guard"
import { askPushGuard } from "@/stores/push-guard"
import { vcsRequestFactory } from "@/stores/repository"

export interface PushOptions {
  /** The remote commit the push replaces; by default the `remoteTip` of the branch's remote status, the row's view */
  expectedRemote?: string | null
  /** A Safe Push an earlier push canceled to restart it on the pushed commit, but did not push (the remote moved) */
  restartRobot?: PatronusRobot
  /**
   * The caller starts a Safe Push right after the push (the Safe Push form): a running Safe Push of the branch is
   * canceled, never restarted, so the branch doesn't run twice
   */
  startsRobot?: boolean
}

/**
 * Pushes a virtual branch and updates its remote status from the result, then the pull requests (the pull request of
 * the branch now shows the pushed commit). A push into a branch whose Safe Push runs asks first (see
 * `guardRunningRobot`). Errors are reported with a toast. Resolves to whether the push succeeded.
 *
 * The push replaces the remote branch only where the row showed it (`expectedRemote`): a remote that moved since is
 * fetched, not overwritten, and a toast says what it has, with a push over exactly that.
 */
export async function pushBranch(branchName: string, options: PushOptions = {}): Promise<boolean> {
  if (isBranchPushing(branchName)) {
    return false
  }

  const branch = getBranch(branchName)
  if (branch === undefined) {
    notifyError(`Cannot push ${branchName}`, "The branch is not in the table: sync and try again.")
    return false
  }

  const guard = await guardRunningRobot(branchName, options.startsRobot === true)
  if (guard.answer === "back") {
    return false
  }
  if (guard.answer === "cancel-push-restart" || guard.answer === "cancel-push") {
    try {
      await cancelPatronusRobot(guard.robot)
    }
    catch (error) {
      notifyError("Cannot cancel the Safe Push", error)
      return false
    }
  }
  const restartRobot = guard.answer === "cancel-push-restart" ? guard.robot : options.restartRobot

  setBranchPushing(branchName, true)
  try {
    const request = vcsRequestFactory.createRequest()
    const progress = new Channel<TransferProgress>()
    progress.onmessage = (pushProgress) => {
      setBranchPushProgress(branchName, pushProgress)
    }
    const result = await commands.pushBranch({
      repositoryPath: request.repositoryPath,
      branchPrefix: request.branchPrefix,
      branchName,
      myEmail: branch.myEmail,
      baselineBranch: branchSyncStore.getState().baselineBranch!,
      // a stacked branch's own commits are those after its parent
      parentBranch: branch.stack?.effectiveBase ?? null,
      // without a loaded status the branch must not be on the remote: if it is, the push fetches and shows it
      expectedRemote: options.expectedRemote === undefined ? branch.remoteStatus?.remoteTip ?? null : options.expectedRemote,
    }, progress)

    if (result.status === "ok") {
      const remoteStatus = toRemoteStatus(result.data.remoteStatus)
      setBranchRemoteStatus(branchName, remoteStatus)
      recordPush(request.repositoryPath, branchName, branch.remoteStatus, result.data.type === "remoteMoved" ? remoteStatus : null)
      if (result.data.type === "remoteMoved") {
        showRemoteMovedToast(branchName, remoteStatus, restartRobot)
        return false
      }
      // in the words of its Activity entry
      const news = describePush(branchName, branch.remoteStatus)
      if (branch.stack === null) {
        // the branch with its new remote status: a branch the push created can have a pull request now
        const action = getPullRequestToastAction(getBranch(branchName) ?? branch)
        showToast({ title: news.title, description: news.description, action: action ?? undefined })
      }
      else {
        showStackedPushToast(branch, news)
      }
      void refreshPullRequestsAfterPush(branchName, branch.commits[0]?.hash)
      if (restartRobot !== undefined) {
        // the restart takes the commit just pushed
        restartAfterPush(restartRobot)
      }
      return true
    }
    notifyError(`Cannot push ${branchName}`, result.error)
  }
  catch (error) {
    notifyError(`Cannot push ${branchName}`, error)
  }
  finally {
    setBranchPushing(branchName, false)
  }
  return false
}

function restartAfterPush(robot: PatronusRobot): void {
  const mode = formatPushMode(robot.pushMode)
  void runRobotAction(restartPatronusRobot, robot, `${mode} restarted`, `Cannot restart the ${mode}`)
}

type RobotGuard = { answer: "push" } | { answer: PushGuardAnswer, robot: PatronusRobot }

/**
 * A push into a branch whose Safe Push runs makes the robot fail at the end ("branch was modified"): the user chooses
 * to cancel it, push and restart it on the pushed commit (only cancel it when the push is for a new Safe Push), to push
 * anyway, or to go back. The status is read fresh first (one row on the server), so a robot that finished meanwhile
 * asks nothing.
 */
async function guardRunningRobot(branchName: string, startsRobot: boolean): Promise<RobotGuard> {
  const robot = getBranchRobot(branchName)
  if (robot === undefined || !isPushIntoRobotRisky(robot, branchName)) {
    return { answer: "push" }
  }
  const status = await fetchPatronusRobotStatus(robot).catch(() => null)
  if (status !== null && status !== "running") {
    return { answer: "push" }
  }
  return { answer: await askPushGuard({ branchName, robot, startsRobot }), robot }
}

/**
 * Pushes a branch at once, unless the push drops remote commits the branch lacks (someone else's work): then the push
 * button of `target` opens its description with the dropped commits, and the Force Push button there pushes with
 * `options`. The keyboard (P in the preview) and the remote-moved toast push through here, as a click on the button does.
 */
export function pushOrConfirm(branchName: string, target: PushConfirmTarget, options: PushOptions = {}): void {
  const remoteStatus = getBranch(branchName)?.remoteStatus ?? null
  if (remoteStatus !== null && dropsRemoteCommits(remoteStatus)) {
    requestPushConfirm(branchName, target, options)
    return
  }
  void pushBranch(branchName, options)
}

/**
 * The remote branch moved since its status was shown, so nothing was pushed: the toast says what the remote has now,
 * and its action pushes over exactly that, through the Safe Push question as any push; a push that drops the commits
 * the remote has now confirms in the row first. A Safe Push the push canceled to restart is restarted by that push.
 */
function showRemoteMovedToast(branchName: string, remoteStatus: RemoteStatus, restartRobot: PatronusRobot | undefined): void {
  const texts = buildRemoteMovedTexts(branchName, remoteStatus)
  if (restartRobot !== undefined && texts.action === null) {
    // nothing left to push: the remote has the branch's commits, the robot can run on them
    restartAfterPush(restartRobot)
  }
  const canceled = restartRobot === undefined || texts.action === null ? "" : ` The ${formatPushMode(restartRobot.pushMode)} is canceled: the push restarts it.`
  const id = toast.add({
    type: "warning",
    title: texts.title,
    description: `${texts.description}${canceled}`,
    // the user decides: the toast stays until then
    timeout: 0,
    ...(texts.action !== null && {
      actionProps: {
        children: texts.action,
        onClick: () => {
          toast.close(id)
          pushOrConfirm(branchName, "row", { expectedRemote: remoteStatus.remoteTip, restartRobot })
        },
      },
    }),
  })
}

/** Pushes the ancestors of a stacked branch that have work for the remote, root first, then the branch; stops at a failure */
export async function pushStack(branchName: string): Promise<void> {
  for (const name of stackPushOrder(branchName, getBranch)) {
    if (!await pushBranch(name)) {
      return
    }
  }
}
