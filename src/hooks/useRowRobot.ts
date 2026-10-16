import type { PatronusRobot } from "@/lib/bindings"
import type { RobotStartCheck, SafePushSlot } from "@/lib/patronus"
import { canStartRobot, getSafePushSlot } from "@/lib/patronus"
import type { Branch } from "@/stores/branch-sync"
import { useBranchRobot, useLandingAncestor, useLandingSpan, usePatronusAvailable, useRobotFailing, useRobotNeedsReview } from "@/stores/patronus"

/** The robot of a branch row, which its Status cell shows and whose next step its Actions cell offers */
export interface RowRobot {
  /** Patronus answered: the repository is on Space */
  readonly available: boolean
  readonly robot: PatronusRobot | undefined
  readonly slot: SafePushSlot
  /** Whether a Safe Push can start (`canStartRobot`) */
  readonly start: RobotStartCheck
  /** The robot failed for missing code-owner approvals */
  readonly needsReview: boolean
}

export function useRowRobot(branch: Branch): RowRobot {
  const available = usePatronusAvailable()
  const robot = useBranchRobot(branch.name)
  const needsReview = useRobotNeedsReview(robot?.id)
  const failing = useRobotFailing(robot?.id)
  const landingAncestor = useLandingAncestor(branch.name)
  const landingSpan = useLandingSpan(branch.name)
  const start = canStartRobot(branch, robot, landingAncestor, landingSpan)
  const slot = getSafePushSlot({
    robot,
    localTip: branch.commits[0]?.hash,
    remote: branch.remoteStatus,
    needsReview,
    failing,
    canOffer: available && !branch.hasError && branch.commitCount > 0,
  })
  return { available, robot, slot, start, needsReview }
}
