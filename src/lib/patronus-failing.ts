import { toast } from "@/components/ui/toast"
import { getPullRequestItem } from "@/hooks/useBranchContextActions"
import type { PatronusRobot } from "@/lib/bindings"
import type { MenuItem } from "@/lib/menu"
import { commands } from "@/lib/bindings"
import { formatFailedCheckNames } from "@/lib/check-counts"
import { announce } from "@/lib/os-notification"
import { describeRobot, findFailedRobots, formatPushMode, openRobot } from "@/lib/patronus"
import { getRobotActions } from "@/lib/patronus-actions"
import { ACTION_TOAST_TIMEOUT_MS } from "@/lib/toast-message"
import { recordRobot } from "@/stores/activity"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore } from "@/stores/branch-sync"
import { patronusStore } from "@/stores/patronus"

/** The failing robots already told about, once each */
const announced = new Set<string>()
/** The failed robots already told about, once each */
const announcedFailures = new Set<string>()
/** The robots that failed meanwhile, until their problems say whether for missing approvals: the key of each */
const pendingFailures = new Map<string, string>()

/** A failed robot is told without its problems after this long: they may not come (offline) */
export const FAILURE_PROBLEMS_WAIT_MS = 15_000

/**
 * Tells once per robot that a running robot will fail (`RepositoryPatronus.failing`): a toast with "Cancel Safe Push"
 * first, then "Open in Patronus", recorded in Activity; and once that a robot failed (`announceFailed`). While no window
 * is focused an OS notification at once (unless Settings turned it off), and the toast waits for the focus. The main
 * window calls it once; it returns the unsubscribe. Its own module, not the store: the actions of a robot
 * (`getRobotActions`) read the store.
 */
export function watchFailingRobots(): () => void {
  const timers = new Set<ReturnType<typeof setTimeout>>()
  const unsubscribe = patronusStore.subscribe((state, previous) => {
    for (const [key, repository] of Object.entries(state.repositories)) {
      const before = previous.repositories[key]
      if (repository.failing !== before?.failing) {
        for (const id of Object.keys(repository.failing)) {
          const robot = repository.robots[id]
          if (robot !== undefined && robot.status === "running" && !announced.has(id)) {
            announced.add(id)
            void announceFailing(key, robot, repository.watched[id])
          }
        }
      }
      if (before !== undefined && repository.robots !== before.robots) {
        for (const robot of findFailedRobots(before.robots, repository.robots)) {
          if (!announcedFailures.has(robot.id) && !pendingFailures.has(robot.id)) {
            pendingFailures.set(robot.id, key)
            const timer = setTimeout(() => {
              timers.delete(timer)
              tellFailed(robot.id, false)
            }, FAILURE_PROBLEMS_WAIT_MS)
            timers.add(timer)
          }
        }
      }
      for (const [id, pendingKey] of pendingFailures) {
        // the problems of a failed robot of a branch say "needs review"; a watched robot of no branch has none read
        if (pendingKey === key && (id in repository.needsReview || repository.watched[id] !== undefined)) {
          tellFailed(id, repository.needsReview[id] === true)
        }
      }
    }
  })
  return () => {
    unsubscribe()
    for (const timer of timers) {
      clearTimeout(timer)
    }
  }
}

function tellFailed(id: string, needsReview: boolean): void {
  const key = pendingFailures.get(id)
  if (key === undefined || announcedFailures.has(id)) {
    return
  }
  pendingFailures.delete(id)
  announcedFailures.add(id)
  const repository = patronusStore.getState().repositories[key]
  const robot = repository?.robots[id]
  if (robot !== undefined) {
    void announceFailed(key, robot, needsReview, repository?.watched[id])
  }
}

/**
 * "Safe Push of auth failed" / "Fast Tests · auth → master", or for missing code-owner approvals "Safe Push of auth
 * needs review" / "The code owners did not approve the changes"
 */
export function formatFailedRobotToast(robot: PatronusRobot, needsReview: boolean, failedChecks: string | null, label?: string): { title: string, description: string } {
  const subject = `${formatPushMode(robot.pushMode)} of ${label ?? robot.branchName ?? robot.name}`
  if (needsReview) {
    return { title: `${subject} needs review`, description: "The code owners did not approve the changes" }
  }
  const target = describeRobot(robot)
  return { title: `${subject} failed`, description: failedChecks === null ? target : `${failedChecks} · ${target}` }
}

/**
 * A robot failed: a toast with its next step (`Restart Safe Push`, or the pull request for the review) and "Open in
 * Patronus", in Activity, and the OS notification of "Safe Push failures" in the background. A robot told failing while
 * it ran is told no second time: its Activity entry says it finished.
 */
async function announceFailed(key: string, robot: PatronusRobot, needsReview: boolean, label: string | undefined): Promise<void> {
  const [repositoryPath, branchPrefix] = key.split("\n") as [string, string]
  if (announced.has(robot.id)) {
    const { title, description } = formatFailedRobotToast(robot, needsReview, null, label)
    recordRobot(title, description, robot, repositoryPath)
    return
  }
  const failedChecks = needsReview ? null : await readFailedChecks(repositoryPath, branchPrefix, robot.id)
  const { title, description } = formatFailedRobotToast(robot, needsReview, failedChecks, label)
  recordRobot(title, description, robot, repositoryPath)
  announce("patronusFailures", { title, body: description }, () => showFailedToast(robot, needsReview, title, description))
}

function showFailedToast(robot: PatronusRobot, needsReview: boolean, title: string, description: string): void {
  // the robot and the branch as the stores have them now: restarted meanwhile, nothing is left to restart
  showRobotToast(robot, needsReview ? "warning" : "error", title, description, (current, branch) => needsReview
    ? (branch === null ? null : getPullRequestItem(branch))
    : getRobotActions(current, branch, false).find(action => action.id === "restart-robot" || action.id === "push-and-restart-robot") ?? null)
}

/** "A required check of auth failed" / "Fast Tests · the Safe Push will fail" */
export function formatFailingRobotToast(robot: PatronusRobot, failedChecks: string | null, label?: string): { title: string, description: string } {
  const consequence = `the ${formatPushMode(robot.pushMode)} will fail`
  return {
    title: `A required check of ${label ?? robot.branchName ?? robot.name} failed`,
    description: failedChecks === null ? consequence : `${failedChecks} · ${consequence}`,
  }
}

async function announceFailing(key: string, robot: PatronusRobot, label: string | undefined): Promise<void> {
  const [repositoryPath, branchPrefix] = key.split("\n") as [string, string]
  // the warning names no check: the details do (cached by the backend for a minute, so the popover reuses them)
  const failedChecks = await readFailedChecks(repositoryPath, branchPrefix, robot.id)
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
 * shows), unless disabled, and "Open in Patronus" below the text (`kind: "robot"`)
 */
function showRobotToast(robot: PatronusRobot, type: "warning" | "error", title: string, description: string, stepOf: (current: PatronusRobot, branch: Branch | null) => MenuItem | null): void {
  const current = currentRobot(robot.id) ?? robot
  const branchName = current.branchName
  const branch = branchName === null ? null : branchSyncStore.getState().branches[branchName] ?? null
  const step = stepOf(current, branch)
  const id = toast.add({
    type,
    title,
    description,
    timeout: ACTION_TOAST_TIMEOUT_MS,
    ...(step !== null && step.disabled !== true && {
      actionProps: {
        children: step.label,
        onClick: () => {
          toast.close(id)
          step.onSelect()
        },
      },
    }),
    data: { kind: "robot", openRobot: () => openRobot(current) },
  })
}

/** The checks of a robot that failed ("Fast Tests"), from its details; null without them */
async function readFailedChecks(repositoryPath: string, branchPrefix: string, robotId: string): Promise<string | null> {
  try {
    const details = await commands.getPatronusRobotDetails({ repositoryPath, branchPrefix, robotId })
    return details.status === "ok" && details.data.checks !== null ? formatFailedCheckNames(details.data.checks) : null
  }
  catch {
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
  pendingFailures.clear()
}
