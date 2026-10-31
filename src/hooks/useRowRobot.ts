import type { PatronusRobot } from "@/lib/bindings"
import type { PatronusChipState } from "@/lib/patronus"
import { getPatronusChipState } from "@/lib/patronus"
import type { Branch } from "@/stores/branch-sync"
import { useRobotFailing, useRobotNeedsReview } from "@/stores/patronus"

/**
 * The chip of the robot of a branch row, in the words of the robot (running, "Failing", "Needs review", the warning dot
 * of a robot of an earlier state of the branch); whether the Status cell shows it Rust decides (`RowStatus`)
 */
export function useRowRobotChip(branch: Branch, robot: PatronusRobot): PatronusChipState {
  const needsReview = useRobotNeedsReview(robot.id)
  const failing = useRobotFailing(robot.id)
  return getPatronusChipState(robot, branch.commits[0]?.hash, branch.remoteStatus, { needsReview, failing })
}
