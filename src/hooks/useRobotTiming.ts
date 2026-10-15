import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import type { PatronusRobot } from "@/lib/bindings"
import type { RobotTiming } from "@/lib/robot-eta"
import { getRobotTiming } from "@/lib/robot-eta"
import { useRobotEstimate } from "@/stores/patronus"

/**
 * The timing of a running robot (`getRobotTiming`), renewed with the relative times: its text changes by the minute, so
 * it is at most 15 s late. Use it only in the small component that shows the time: each tick renders it again.
 */
export function useRobotTiming(robot: Pick<PatronusRobot, "pushMode" | "targetBranch" | "startTime" | "profile">): RobotTiming {
  const estimate = useRobotEstimate(robot)
  const now = useNow(RELATIVE_TIME_TICK_MS)
  return getRobotTiming(robot, estimate, now)
}
