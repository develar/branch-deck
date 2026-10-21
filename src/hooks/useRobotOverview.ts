import { useMemo } from "react"
import { useIsFullyLanded } from "@/hooks/useLandedIn"
import { useNow } from "@/hooks/useNow"
import type { RobotOverview } from "@/lib/robot-overview"
import { collectRobotOverview } from "@/lib/robot-overview"
import type { Branch, StackBranch } from "@/stores/branch-sync"
import { useBranchSync, useBranchTableRows } from "@/stores/branch-sync"
import { getLandingAncestor, getStackLandingMember, useFailingRobots, usePushHolds, useRepositoryRobots, useRobotsNeedingReview, useWatchedRobots } from "@/stores/patronus"

/** The last day of finished robots moves on by the minute: a robot that finished 24 hours ago drops out within one */
const FINISHED_TICK_MS = 60_000

const noBranches: Readonly<Record<string, Branch>> = {}
const noStacks: Readonly<Record<string, StackBranch>> = {}

/**
 * The robots of the selected repository's branches, of its whole stacks and the watched ones (`collectRobotOverview`),
 * for the landing overview and the Dock
 */
export function useRobotOverview(): RobotOverview {
  const rows = useBranchTableRows()
  const robots = useRepositoryRobots()
  // without robots (no Patronus) the overview is empty: no re-render on every change of a branch
  const branches = useBranchSync(state => (robots === undefined ? noBranches : state.branches))
  const stacks = useBranchSync(state => (robots === undefined ? noStacks : state.stackBranches))
  const needsReview = useRobotsNeedingReview()
  const watched = useWatchedRobots()
  const failing = useFailingRobots()
  const holds = usePushHolds()
  const isLanded = useIsFullyLanded()
  const now = useNow(FINISHED_TICK_MS)
  return useMemo(() => {
    const order = rows.flatMap(row => (row.kind === "branch" ? [row.name] : []))
    // another robot holds, lands a part of or landed it, or the baseline has it: its old failure asks for nothing. The
    // robots and the branches the landing ones are found by are deps of this memo
    const isCarried = (name: string) => holds.has(name) || isLanded(name) || getLandingAncestor(name) !== null
    const isStackCarried = (root: string) => getStackLandingMember(root) !== null
    return collectRobotOverview(order, branches, robots, needsReview, now, watched, failing, stacks, isCarried, isStackCarried)
  }, [rows, branches, robots, needsReview, now, watched, failing, stacks, holds, isLanded])
}
