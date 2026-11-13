import type { PatronusRobot } from "@/lib/bindings"
import type { PatronusChipState } from "@/lib/patronus"
import { getPatronusChipState } from "@/lib/patronus"
import { useRowPart } from "@/model/store"
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
 * whose robot ran on it. The row, the title bar and the hover of a node show only a running robot or one of the subject
 * as it is, whose dot the remote status measures as Rust does: no earlier state to read (`useEarlierRobot`)
 */
export function useRobotChip(robot: PatronusRobot, tip: string | undefined, remote: RemoteStatus | null): PatronusChipState {
  const needsReview = useRobotNeedsReview(robot.id)
  const failing = useRobotFailing(robot.id)
  return getPatronusChipState(robot, tip, remote, { needsReview, failing })
}

/**
 * Rust found `robotId` the robot of an earlier state of `subject` (a branch, `stack:<root>`; `NextStep.earlierRobot`):
 * true, else undefined, where the remote status tells (`RobotFlags.earlier`). The surfaces that show such a robot read
 * it: the preview and the details of a robot
 */
export function useEarlierRobot(subject: string, robotId: string | undefined): true | undefined {
  const earlier = useRowPart(subject, "landing")?.step.earlierRobot
  return robotId !== undefined && earlier === robotId ? true : undefined
}
