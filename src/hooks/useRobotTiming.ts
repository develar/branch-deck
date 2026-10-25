import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import type { PatronusRobot } from "@/lib/bindings"
import type { RobotTiming } from "@/lib/robot-eta"
import { getRobotTiming, withCheckProgress } from "@/lib/robot-eta"
import { useRobotEstimate } from "@/stores/patronus"
import { useReadRobotProgress } from "@/stores/robot-details"

/**
 * The timing of a running robot (`getRobotTiming`), renewed with the relative times: its text changes by the minute, so
 * it is at most 15 s late. With `checks`, the time left the CI estimated while the robot's details are open replaces the
 * one from history (`withCheckProgress`); the slot of the row and its progress line take it, the details' own line and
 * the Dock don't. Use it only in the small component that shows the time: each tick renders it again.
 */
export function useRobotTiming(robot: Pick<PatronusRobot, "id" | "pushMode" | "targetBranch" | "startTime" | "profile">, checks = false): RobotTiming {
  const estimate = useRobotEstimate(robot)
  const read = useReadRobotProgress(robot.id)
  const now = useNow(RELATIVE_TIME_TICK_MS)
  const timing = getRobotTiming(robot, estimate, now)
  return checks ? withCheckProgress(timing, read, now) : timing
}
