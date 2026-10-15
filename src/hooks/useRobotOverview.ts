import { useMemo } from "react"
import { useNow } from "@/hooks/useNow"
import type { RobotOverview } from "@/lib/robot-overview"
import { collectRobotOverview } from "@/lib/robot-overview"
import { useBranchSync, useBranchTableRows } from "@/stores/branch-sync"
import { useRepositoryRobots, useRobotsNeedingReview } from "@/stores/patronus"

/** The last day of finished robots moves on by the minute: a robot that finished 24 hours ago drops out within one */
const FINISHED_TICK_MS = 60_000

/** The robots of the selected repository's branches (`collectRobotOverview`), for the robots indicator and the Dock */
export function useRobotOverview(): RobotOverview {
  const rows = useBranchTableRows()
  const branches = useBranchSync(state => state.branches)
  const robots = useRepositoryRobots()
  const needsReview = useRobotsNeedingReview()
  const now = useNow(FINISHED_TICK_MS)
  return useMemo(() => {
    const order = rows.flatMap(row => row.kind === "branch" ? [row.name] : [])
    return collectRobotOverview(order, branches, robots, needsReview, now)
  }, [rows, branches, robots, needsReview, now])
}
