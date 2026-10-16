import { useMemo } from "react"
import { useNow } from "@/hooks/useNow"
import type { RobotOverview } from "@/lib/robot-overview"
import { collectRobotOverview } from "@/lib/robot-overview"
import type { Branch } from "@/stores/branch-sync"
import { useBranchSync, useBranchTableRows } from "@/stores/branch-sync"
import { useFailingRobots, useRepositoryRobots, useRobotsNeedingReview, useWatchedRobots } from "@/stores/patronus"

/** The last day of finished robots moves on by the minute: a robot that finished 24 hours ago drops out within one */
const FINISHED_TICK_MS = 60_000

const noBranches: Readonly<Record<string, Branch>> = {}

/**
 * The robots of the selected repository's branches and the watched ones (`collectRobotOverview`), for the robots
 * indicator and the Dock
 */
export function useRobotOverview(): RobotOverview {
  const rows = useBranchTableRows()
  const robots = useRepositoryRobots()
  // without robots (no Patronus) the overview is empty: no re-render on every change of a branch
  const branches = useBranchSync(state => (robots === undefined ? noBranches : state.branches))
  const needsReview = useRobotsNeedingReview()
  const watched = useWatchedRobots()
  const failing = useFailingRobots()
  const now = useNow(FINISHED_TICK_MS)
  return useMemo(() => {
    const order = rows.flatMap(row => (row.kind === "branch" ? [row.name] : []))
    return collectRobotOverview(order, branches, robots, needsReview, now, watched, failing)
  }, [rows, branches, robots, needsReview, now, watched, failing])
}
