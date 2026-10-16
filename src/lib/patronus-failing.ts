import { toast } from "@/components/ui/toast"
import type { PatronusRobot } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { formatFailedCheckNames } from "@/lib/check-counts"
import { announce } from "@/lib/os-notification"
import { formatPushMode, openRobot } from "@/lib/patronus"
import { getRobotActions } from "@/lib/patronus-actions"
import { ACTION_TOAST_TIMEOUT_MS } from "@/lib/toast-message"
import { recordRobot } from "@/stores/activity"
import { branchSyncStore } from "@/stores/branch-sync"
import { patronusStore } from "@/stores/patronus"

/** The failing robots already told about, once each */
const announced = new Set<string>()

/**
 * Tells once per robot that a running robot will fail (`RepositoryPatronus.failing`): a toast with "Cancel Safe Push"
 * first, then "Open in Patronus", recorded in Activity; while no window is focused an OS notification at once (unless
 * Settings turned it off), and the toast waits for the focus. The main window calls it once; it returns the unsubscribe.
 * Its own module, not the store: the actions of a robot (`getRobotActions`) read the store.
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
  let failedChecks: string | null = null
  try {
    const details = await commands.getPatronusRobotDetails({ repositoryPath, branchPrefix, robotId: robot.id })
    if (details.status === "ok" && details.data.checks !== null) {
      failedChecks = formatFailedCheckNames(details.data.checks)
    }
  }
  catch {
    // the toast says it without the names
  }
  const { title, description } = formatFailingRobotToast(robot, failedChecks, label)
  recordRobot(title, description, robot, repositoryPath)
  announce("patronusFailures", { title, body: description }, () => showFailingToast(robot, title, description))
}

function showFailingToast(robot: PatronusRobot, title: string, description: string): void {
  // the robot as the store has it now: canceled meanwhile, it has nothing to cancel
  const current = currentRobot(robot.id) ?? robot
  const branchName = current.branchName
  const branch = branchName === null ? null : branchSyncStore.getState().branches[branchName] ?? null
  const cancel = getRobotActions(current, branch).find(action => action.id === "cancel-robot")
  const id = toast.add({
    type: "warning",
    title,
    description,
    timeout: ACTION_TOAST_TIMEOUT_MS,
    ...(cancel !== undefined && {
      actionProps: {
        children: cancel.label,
        onClick: () => {
          toast.close(id)
          cancel.onSelect?.()
        },
      },
    }),
    data: { kind: "robot", openRobot: () => openRobot(current) },
  })
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
}
