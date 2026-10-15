import type { PatronusRobot } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { describeRobot, openRobot } from "@/lib/patronus"
import { showToast } from "@/lib/toast-message"
import { recordRobot } from "@/stores/activity"

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
