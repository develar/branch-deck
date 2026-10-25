import { toast } from "@/components/ui/toast"
import { getPullRequestItem } from "@/hooks/useBranchContextActions"
import { getWholeStackMenuItems } from "@/hooks/useWholeStackActions"
import type { News, PatronusRobot, RobotFailureKind } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { formatFailedCheckNames } from "@/lib/check-counts"
import type { MenuItem } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import { announce } from "@/lib/os-notification"
import { describeRobot, formatPushMode, formatRobotSubject, openRobot } from "@/lib/patronus"
import { getRobotActions } from "@/lib/patronus-actions"
import { copyRobotPrompt, robotAncestors } from "@/lib/robot-prompt-copy"
import { ACTION_TOAST_TIMEOUT_MS, showToast } from "@/lib/toast-message"
import { recordRobot } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore } from "@/stores/branch-sync"
import { announceMerge, getBranchRobot, getStackRobot, patronusStore, repositoryKey } from "@/stores/patronus"
import { getEffectiveBranchPrefix } from "@/stores/repository"

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
 * (`announceMerge`), a robot failed, told once Patronus said whether for missing code-owner approvals (or its problems
 * didn't come, or the session reads none of a robot of no branch: then as a plain failure, `announceFailed`)
 */
export function tellRobotNews(repositoryPath: string, news: readonly News[]): void {
  const key = repositoryKey(repositoryPath, getEffectiveBranchPrefix())
  for (const { event } of news) {
    if (event.kind === "approvalsArrived") {
      continue
    }
    const repository = patronusStore.getState().repositories[key]
    const robot = repository?.robots[event.robotId]
    if (robot === undefined) {
      continue
    }
    if (event.kind === "robotMerged") {
      announceMerge(key, robot)
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
 * no second time: its Activity entry says it finished.
 */
async function announceFailed(key: string, robot: PatronusRobot, needsReview: boolean, label: string | undefined): Promise<void> {
  const [repositoryPath] = key.split("\n") as [string, string]
  if (announced.has(robot.id)) {
    const { title, description } = formatFailedRobotToast(robot, needsReview, null, label)
    recordRobot(title, description, robot, repositoryPath)
    return
  }
  // the failure also warms the backend's cache of the details, so the prompt copies at once
  const [failedChecks, kind] = needsReview ? [null, null] : await Promise.all([readFailedChecks(repositoryPath, robot.id), readFailureKind(repositoryPath, robot)])
  const { title, description } = formatFailedRobotToast(robot, needsReview, failedChecks, label, kind)
  recordRobot(title, description, robot, repositoryPath)
  announce("patronusFailures", { title, body: description }, () => showFailedToast(robot, needsReview, kind, title, description))
}

function showFailedToast(robot: PatronusRobot, needsReview: boolean, kind: RobotFailureKind | null, title: string, description: string): void {
  // the robot and the branch as the stores have them now: restarted meanwhile, nothing is left to restart
  showRobotToast(robot, needsReview ? "warning" : "error", title, description, (current, branch) => {
    if (needsReview) {
      // the approvals of the whole stack count in the pull request of its stack branch
      if (current.stackRoot != null) {
        return getWholeStackMenuItems(current.stackRoot, { withPush: false }).find(item => item.id === "open-stack-pull-request" || item.id === "create-stack-pull-request") ?? null
      }
      return branch === null ? null : getPullRequestItem(branch)
    }
    const actions = getRobotActions(current, branch, false, false)
    const restart = actions.find(action => action.id === "restart-robot" || action.id === "push-and-restart-robot") ?? null
    return kind === "infrastructure" ? restart : (actions.find(action => action.id === "copy-robot-prompt") ?? restart)
  })
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
  const failedChecks = await readFailedChecks(repositoryPath, robot.id)
  const { title, description } = formatFailingRobotToast(robot, failedChecks, label)
  recordRobot(title, description, robot, repositoryPath)
  announce("patronusFailures", { title, body: description }, () => showFailingToast(robot, title, description))
}

function showFailingToast(robot: PatronusRobot, title: string, description: string): void {
  // the robot as the store has it now: canceled meanwhile, it has nothing to cancel
  showRobotToast(robot, "warning", title, description, (current, branch) => getRobotActions(current, branch).find(action => action.id === "cancel-robot") ?? null)
}

/**
 * The toast of a robot: its step as the action (`stepOf` the robot and its branch as the stores have them when it
 * shows), unless disabled, and "Open in Patronus" below the text (`secondaryAction`)
 */
function showRobotToast(
  robot: PatronusRobot,
  type: "warning" | "error",
  title: string,
  description: string,
  stepOf: (current: PatronusRobot, branch: Branch | null) => MenuItem | null,
): void {
  const current = currentRobot(robot.id) ?? robot
  const branchName = current.branchName
  const branchOf = (name: string | null): Branch | null => (name === null ? null : (branchSyncStore.getState().branches[name] ?? null))
  const step = stepOf(current, branchOf(branchName))
  /**
   * The step as the stores have it at the click; null after an info toast said why it no longer applies. A step on the
   * robot is checked (`ROBOT_STEPS`); the pull request of the branch opens whatever the robot did since.
   */
  const stepAtClick = (offered: MenuItem): MenuItem | null => {
    const latest = currentRobot(robot.id) ?? current
    const now = stepOf(latest, branchOf(latest.branchName))
    if (!ROBOT_STEPS.has(offered.id ?? "")) {
      return now ?? offered
    }
    // a restart is a newer robot of the branch, or of the whole stack
    const newer = latest.branchName !== null ? getBranchRobot(latest.branchName) : latest.stackRoot != null ? getStackRobot(latest.stackRoot) : undefined
    const stale = formatStaleRobotStep(offered, now, current, latest, newer !== undefined && newer.id !== latest.id && newer.startTime > latest.startTime ? newer : null)
    if (stale === null) {
      return now
    }
    showToast({ type: "info", ...stale, action: { children: "Open in Patronus", onClick: () => openRobot(newer ?? latest) } })
    return null
  }
  const id = toast.add({
    type,
    title,
    description,
    timeout: ACTION_TOAST_TIMEOUT_MS,
    ...(step !== null &&
      step.disabled !== true && {
        actionProps: {
          children: step.label,
          onClick:
            step.id === "copy-robot-prompt"
              ? // the toast says in place that it copied: the clipboard shows nothing
                () => {
                  if (stepAtClick(step) === null) {
                    toast.close(id)
                    return
                  }
                  void copyRobotPrompt(current).then(copied => {
                    if (copied) {
                      toast.update(id, {
                        type: "success",
                        title: `Copied the prompt for ${formatRobotSubject(current)}`,
                        description: "Paste it into your coding agent",
                        actionProps: undefined,
                      })
                    } else {
                      toast.close(id)
                    }
                  })
                }
              : () => {
                  toast.close(id)
                  // decided when the toast showed: a restart, a cancel or a finish since leaves nothing to do
                  stepAtClick(step)?.onSelect(FROM_MENU)
                },
        },
      }),
    data: { secondaryAction: { label: "Open in Patronus", onClick: () => openRobot(current) } },
  })
}

/** The steps of a robot toast that act on the robot, decided again at the click */
const ROBOT_STEPS: ReadonlySet<string> = new Set(["restart-robot", "push-and-restart-robot", "cancel-robot", "copy-robot-prompt"])

/** How a robot's status reads after its subject: "Safe Push of auth merged" */
function formatRobotStatus(robot: PatronusRobot): string {
  switch (robot.status) {
    case "running":
      return "is running"
    case "success":
      return robot.pushMode === "dryRun" ? "passed" : "merged"
    case "failure":
      return "failed"
    case "canceled":
      return "was canceled"
    case "canceling":
      return "is canceling"
    case "unknown":
      return "changed"
  }
}

/**
 * Why the step a robot toast offered (`offered`, for the robot as it was then, `shown`) no longer applies at the click,
 * in the words of an info toast (a precondition, nothing failed): a newer robot of the branch (restarted meanwhile), a
 * robot that finished or was canceled since, or a step disabled now (`now`, with its reason). Null while it applies.
 */
export function formatStaleRobotStep(
  offered: MenuItem,
  now: MenuItem | null,
  shown: PatronusRobot,
  current: PatronusRobot,
  newer: PatronusRobot | null,
): { title: string; description: string } | null {
  if (newer === null && now !== null && now.id === offered.id && now.disabled !== true) {
    return null
  }
  const subject = `${formatPushMode(current.pushMode)} of ${formatRobotSubject(current)}`
  if (newer !== null) {
    return { title: `${subject} was restarted`, description: describeRobot(newer) }
  }
  if (current.status !== shown.status) {
    return { title: `${subject} ${formatRobotStatus(current)}`, description: `${offered.label} no longer applies` }
  }
  if (now !== null && now.id === offered.id) {
    return { title: `${offered.label} isn't possible now`, description: now.description ?? describeRobot(current) }
  }
  return { title: `${offered.label} no longer applies`, description: now === null ? describeRobot(current) : `${now.label} is the next step` }
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

/** The checks of a robot that failed ("Fast Tests"), from its details; null without them */
async function readFailedChecks(repositoryPath: string, robotId: string): Promise<string | null> {
  try {
    const details = await commands.getPatronusRobotDetails({ repositoryPath, robotId })
    return details.status === "ok" && details.data.checks !== null ? formatFailedCheckNames(details.data.checks) : null
  } catch {
    // the toast says it without the names
    return null
  }
}

function currentRobot(id: string): PatronusRobot | undefined {
  for (const repository of Object.values(patronusStore.getState().repositories)) {
    const robot = repository.robots[id]
    if (robot !== undefined) {
      return robot
    }
  }
  return undefined
}

/** Forgets the robots told about (tests) */
export function resetFailingAnnouncements(): void {
  announced.clear()
  announcedFailures.clear()
}
