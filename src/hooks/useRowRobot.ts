import type { PatronusRobot } from "@/lib/bindings"
import type { SafePushSlot } from "@/lib/patronus"
import { getSafePushSlot, isLandedByOwnRobot } from "@/lib/patronus"
import type { RobotStartCheck } from "@/presenters/reasons"
import { startCheck } from "@/presenters/reasons"
import type { Branch } from "@/stores/branch-sync"
import { useBranchLanding, useBranchRobot, useLandedBy, usePatronusAvailable, useRobotFailing, useRobotNeedsReview } from "@/stores/patronus"

/** The robot of a branch row, which its Status cell shows and whose next step its Actions cell offers */
export interface RowRobot {
  /** Patronus answered: the repository is on Space */
  readonly available: boolean
  readonly robot: PatronusRobot | undefined
  readonly slot: SafePushSlot
  /** Whether a Safe Push can start (`landing.start` of the view, `domain_policy::land::start_refusal`) */
  readonly start: RobotStartCheck
  /** The robot failed for missing code-owner approvals */
  readonly needsReview: boolean
}

export function useRowRobot(branch: Branch): RowRobot {
  const available = usePatronusAvailable()
  const robot = useBranchRobot(branch.name)
  const needsReview = useRobotNeedsReview(robot?.id)
  const failing = useRobotFailing(robot?.id)
  const landedBy = useLandedBy(branch.name)
  const start = startCheck(useBranchLanding(branch.name).start)
  const slot = getSafePushSlot({
    robot,
    localTip: branch.commits[0]?.hash,
    remote: branch.remoteStatus,
    needsReview,
    failing,
    canOffer: available && !branch.hasError && branch.commitCount > 0,
    landed: isLandedByOwnRobot(landedBy, robot),
  })
  return { available, robot, slot, start, needsReview }
}
