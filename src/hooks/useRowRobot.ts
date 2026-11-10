import type { PatronusRobot } from "@/lib/bindings"
import type { PatronusChipState } from "@/lib/patronus"
import { getPatronusChipState } from "@/lib/patronus"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import { useRobotFailing, useRobotNeedsReview } from "@/stores/patronus"

/**
 * The chip of the robot of a branch row, in the words of the robot (running, "Failing", "Needs review", the warning dot
 * of a robot of an earlier state of the branch); whether the Status cell shows it Rust decides (`RowStatus`)
 */
export function useRowRobotChip(branch: Branch, robot: PatronusRobot): PatronusChipState {
  return useRobotChip(robot, branch.commits[0]?.hash, branch.remoteStatus)
}

/**
 * The chip of `robot` of the branch whose tip and remote are these: of a branch, or of the stack branch of a whole stack,
 * whose robot ran on it
 */
export function useRobotChip(robot: PatronusRobot, tip: string | undefined, remote: RemoteStatus | null): PatronusChipState {
  const needsReview = useRobotNeedsReview(robot.id)
  const failing = useRobotFailing(robot.id)
  return getPatronusChipState(robot, tip, remote, { needsReview, failing })
}
