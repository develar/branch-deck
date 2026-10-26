import { getPullRequestItem } from "@/hooks/useBranchContextActions"
import { getWholeStackMenuItems } from "@/hooks/useWholeStackActions"
import type { News, PatronusRobot, RobotFailureKind } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { formatFailedCheckNames } from "@/lib/check-counts"
import { closeEventToasts, isEventToastOpen, replaceEventToast, toastKeys } from "@/lib/event-toast"
import type { MenuItem } from "@/lib/menu"
import { announce } from "@/lib/os-notification"
import { describeRobot, formatPushMode, formatRobotSubject } from "@/lib/patronus"
import { copyRobotPrompt, robotAncestors } from "@/lib/robot-prompt-copy"
import { robotToastKeys, showRobotEventToast } from "@/lib/robot-toast"
import { getRobotActionItems, isRestart } from "@/presenters/actions"
import { recordRobot } from "@/stores/activity"
import { branchSyncStore } from "@/stores/branch-sync"
import { announceMerge, patronusStore, repositoryKey } from "@/stores/patronus"
import { getEffectiveBranchPrefix } from "@/stores/repository"
import { readRobotDetails } from "@/stores/robot-details"

/** The failing robots already told about, once each */
const announced = new Set<string>()
/** The failed robots already told about, once each */
const announcedFailures = new Set<string>()

/**
 * Tells once per robot that a running robot will fail (`RepositoryPatronus.failing`, which the session asks): a toast
 * with "Cancel Safe Push" first, then "Open in Patronus", recorded in Activity. While no window is focused an OS
 * notification at once (unless Settings turned it off), and the toast waits for the focus. The main window calls it
 * once; it returns the unsubscribe. Its own module, not the store: the actions of a robot (`getRobotActions`) read the
 * store.
 */
export function watchFailingRobots(): () => void {
  return patronusStore.subscribe((state, previous) => {
    for (const [key, repository] of Object.entries(state.repositories)) {
      if (repository.failing === previous.repositories[key]?.failing) {
        continue
      }
      for (const id of Object.keys(repository.failing)) {
        const robot = repository.robots[id]
        if (robot !== undefined && robot.status === "running" && !announced.has(id)) {
          announced.add(id)
          void announceFailing(key, robot, repository.watched[id])
        }
      }
    }
  })
}

/**
 * The news of the session of `repositoryPath` the window has not told (`onSessionFacts` hands each once): a robot merged
 * (`announceMerge`, with the pull requests its landing closes), a robot failed, told once Patronus said whether for missing code-owner approvals (or its problems
 * didn't come, or the session reads none of a robot of no branch: then as a plain failure, `announceFailed`)
 */
export function tellRobotNews(repositoryPath: string, news: readonly News[]): void {
  const key = repositoryKey(repositoryPath, getEffectiveBranchPrefix())
  for (const { event } of news) {
    // the other news are of the review requests and the approvals
    if (event.kind !== "robotMerged" && event.kind !== "robotFailed") {
      continue
    }
    const repository = patronusStore.getState().repositories[key]
    const robot = repository?.robots[event.robotId]
    if (robot === undefined) {
      continue
    }
    if (event.kind === "robotMerged") {
      announceMerge(key, robot, event.closes)
    } else if (!announcedFailures.has(robot.id)) {
      announcedFailures.add(robot.id)
      void announceFailed(key, robot, event.needsReview === true, repository?.watched[robot.id])
    }
  }
}

/**
 * "Safe Push of auth failed" / "Fast Tests · auth → master" ("Safe Push of the whole air stack failed"), or for missing
 * code-owner approvals "Safe Push of auth needs review" / "The code owners did not approve the changes" (with a
 * profile: "Large Scale Change doesn't skip the code-owner review: …"). A massive failure says to look at the target
 * first: "Fast Tests · many tests failed, check master · auth → master"
 */
export function formatFailedRobotToast(
  robot: PatronusRobot,
  needsReview: boolean,
  failedChecks: string | null,
  label?: string,
  kind: RobotFailureKind | null = null,
): { title: string; description: string } {
  const subject = `${formatPushMode(robot.pushMode)} of ${formatRobotSubject(robot, label)}`
  if (needsReview) {
    // a profile chosen to start without the approvals that checks them all the same: Patronus tells it only now
    const profile = robot.profile?.name
    const description =
      profile == null ? "The code owners did not approve the changes" : `${profile} doesn't skip the code-owner review: the code owners did not approve the changes`
    return { title: `${subject} needs review`, description }
  }
  const parts = [...(failedChecks === null ? [] : [failedChecks]), ...(kind === "massive" ? [`many tests failed, check ${robot.targetBranch}`] : []), describeRobot(robot)]
  return { title: `${subject} failed`, description: parts.join(" · ") }
}

/**
 * A robot failed (the news of the session): a toast with its next step and "Open in Patronus", in Activity, and the OS notification of "Safe Push
 * failures" in the background. The step is "Copy Prompt for Agent", "Restart Safe Push" when only the CI failed (its
 * builds never started, `infrastructure`), or the pull request for the review. A robot told failing while it ran is told
 * no second time: its Activity entry says it finished, and its toast, while still open, says in place that it failed.
 * The toast is the robot's (`showRobotEventToast`): it takes the place of the start's, and a restart takes its place.
 */
async function announceFailed(key: string, robot: PatronusRobot, needsReview: boolean, label: string | undefined): Promise<void> {
  const [repositoryPath] = key.split("\n") as [string, string]
  const toldFailing = announced.has(robot.id)
  if (toldFailing) {
    const { title, description } = formatFailedRobotToast(robot, needsReview, null, label)
    recordRobot(title, description, robot, repositoryPath)
    if (!isEventToastOpen(toastKeys.robot(repositoryPath, robot.id))) {
      return
    }
  }
  // the failure also warms the backend's cache of the details, so the prompt copies at once
  const [failedChecks, kind] = needsReview ? [null, null] : await Promise.all([readFailedChecks(robot.id), readFailureKind(repositoryPath, robot)])
  const { title, description } = formatFailedRobotToast(robot, needsReview, failedChecks, label, kind)
  if (toldFailing) {
    showFailedToast(repositoryPath, robot, needsReview, kind, title, description, true)
    return
  }
  recordRobot(title, description, robot, repositoryPath)
  announce("patronusFailures", { title, body: description }, () => showFailedToast(repositoryPath, robot, needsReview, kind, title, description, false))
}

function showFailedToast(
  repositoryPath: string,
  robot: PatronusRobot,
  needsReview: boolean,
  kind: RobotFailureKind | null,
  title: string,
  description: string,
  replaceOnly: boolean,
): void {
  showRobotEventToast({
    repositoryPath,
    robot,
    type: needsReview ? "warning" : "error",
    title,
    description,
    replaceOnly,
    // the robot and the branch as the stores have them now: restarted meanwhile, nothing is left to restart
    stepOf: (current, branch) => {
      if (needsReview) {
        // the approvals of the whole stack count in the pull request of its stack branch
        if (current.stackRoot != null) {
          return (
            getWholeStackMenuItems(current.stackRoot, { withPush: false }).find(item => item.id === "open-stack-pull-request" || item.id === "create-stack-pull-request") ?? null
          )
        }
        return branch === null ? null : getPullRequestItem(branch)
      }
      // as Rust offers them now: a restart, the prompt of a robot that didn't fail for the approvals
      const actions = getRobotActionItems(current)
      const restart = actions.find(isRestart) ?? null
      const copy = actions.find(action => action.id === "copy-robot-prompt")
      return kind === "infrastructure" || copy === undefined ? restart : copyInPlace(repositoryPath, copy, current)
    },
  })
}

/**
 * "Copy Prompt for Agent" of a toast: the toast says in place that it copied ("Copied the prompt for auth" / "Paste it
 * into your coding agent"), as the clipboard shows nothing, and closes when it didn't
 */
function copyInPlace(repositoryPath: string, copy: MenuItem, robot: PatronusRobot): MenuItem {
  return {
    ...copy,
    onSelect: () =>
      void copyRobotPrompt(robot).then(copied => {
        const keys = robotToastKeys(repositoryPath, robot)
        if (copied) {
          replaceEventToast({
            ...keys,
            render: () => ({ type: "success", title: `Copied the prompt for ${formatRobotSubject(robot)}`, description: "Paste it into your coding agent" }),
          })
        } else {
          closeEventToasts(keys.key)
        }
      }),
  }
}

/** "A required check of auth failed" / "Fast Tests · the Safe Push will fail" ("… of the whole air stack failed") */
export function formatFailingRobotToast(robot: PatronusRobot, failedChecks: string | null, label?: string): { title: string; description: string } {
  const consequence = `the ${formatPushMode(robot.pushMode)} will fail`
  return {
    title: `A required check of ${formatRobotSubject(robot, label)} failed`,
    description: failedChecks === null ? consequence : `${failedChecks} · ${consequence}`,
  }
}

async function announceFailing(key: string, robot: PatronusRobot, label: string | undefined): Promise<void> {
  const [repositoryPath] = key.split("\n") as [string, string]
  // the warning names no check: the details do (cached by the backend for a minute, so the popover reuses them)
  const failedChecks = await readFailedChecks(robot.id)
  const { title, description } = formatFailingRobotToast(robot, failedChecks, label)
  recordRobot(title, description, robot, repositoryPath)
  announce("patronusFailures", { title, body: description }, () =>
    // the robot as the store has it now: canceled meanwhile, it has nothing to cancel
    showRobotEventToast({
      repositoryPath,
      robot,
      type: "warning",
      title,
      description,
      stepOf: current => getRobotActionItems(current).find(action => action.id === "cancel-robot") ?? null,
    }),
  )
}

/** What failed, which decides the step of the toast; null when Patronus can't tell */
async function readFailureKind(repositoryPath: string, robot: PatronusRobot): Promise<RobotFailureKind | null> {
  try {
    const baselineBranch = branchSyncStore.getState().baselineBranch ?? ""
    const result = await commands.getRobotFailure({
      repositoryPath,
      robotId: robot.id,
      branchName: robot.branchName,
      // the members of the whole stack for its robot
      ancestors: robotAncestors(robot),
      baselineBranch,
      full: false,
    })
    return result.status === "ok" ? result.data.failure.kind : null
  } catch {
    // the toast offers the prompt
    return null
  }
}

/** The checks of a robot that failed ("Fast Tests"), from its details as the session reads them; null without them */
async function readFailedChecks(robotId: string): Promise<string | null> {
  const details = await readRobotDetails(robotId)
  // without them the toast says it without the names
  return details?.checks == null ? null : formatFailedCheckNames(details.checks)
}

/** Forgets the robots told about (tests) */
export function resetFailingAnnouncements(): void {
  announced.clear()
  announcedFailures.clear()
}
