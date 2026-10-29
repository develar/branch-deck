import type { PatronusRobot } from "@/lib/bindings"
import type { SafePushSlot } from "@/lib/patronus"
import { getSafePushSlot, isLandedByOwnRobot } from "@/lib/patronus"
import type { Branch } from "@/stores/branch-sync"
import { useLandedBy, usePatronusAvailable, useRobotFailing, useRobotNeedsReview } from "@/stores/patronus"

/**
 * The slot of the robot of a branch row, which its Status cell shows (`BranchRowStatus`): running, finished on the
 * branch as it is, or a start; only a row with a robot asks it
 */
export function useRowRobotSlot(branch: Branch, robot: PatronusRobot): SafePushSlot {
  const available = usePatronusAvailable()
  const needsReview = useRobotNeedsReview(robot.id)
  const failing = useRobotFailing(robot.id)
  const landedBy = useLandedBy(branch.name)
  return getSafePushSlot({
    robot,
    localTip: branch.commits[0]?.hash,
    remote: branch.remoteStatus,
    needsReview,
    failing,
    canOffer: available && !branch.hasError && branch.commitCount > 0,
    landed: isLandedByOwnRobot(landedBy, robot),
  })
}
