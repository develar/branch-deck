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
  return useRobotChip(robot, branch.name, branch.commits[0]?.hash, branch.remoteStatus)
}

/**
 * The chip of `robot` of `subject` (a branch, `stack:<root>` of a whole stack, whose robot ran on its stack branch), whose
 * tip and remote are these
 */
export function useRobotChip(robot: PatronusRobot, subject: string, tip: string | undefined, remote: RemoteStatus | null): PatronusChipState {
  const needsReview = useRobotNeedsReview(robot.id)
  const failing = useRobotFailing(robot.id)
  const earlier = useEarlierRobot(subject, robot.id)
  return getPatronusChipState(robot, tip, remote, { needsReview, failing, earlier })
}

/**
 * Rust found `robotId` the robot of an earlier state of `subject` (a branch, `stack:<root>`; `NextStep.earlierRobot`):
 * true, else undefined, where the remote status tells (`RobotFlags.earlier`)
 */
export function useEarlierRobot(subject: string, robotId: string | undefined): true | undefined {
  const earlier = useRowPart(subject, "landing")?.step.earlierRobot
  return robotId !== undefined && earlier === robotId ? true : undefined
}
