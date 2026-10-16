import { useMemo } from "react"
import { useLandedIn } from "@/hooks/useLandedIn"
import type { LaneNode } from "@/hooks/useStatusBadge"
import { getLaneNode } from "@/hooks/useStatusBadge"
import { needsRobotAttention } from "@/lib/patronus"
import type { Branch } from "@/stores/branch-sync"
import { useBranchRobot, useRobotNeedsReview } from "@/stores/patronus"

/** The node of a branch in the stack graph (`getLaneNode`), in the colour of a robot that failed on the branch as it is */
export function useLaneNode(branch: Branch): LaneNode {
  const landedIn = useLandedIn(branch.name)
  const robot = useBranchRobot(branch.name)
  const needsReview = useRobotNeedsReview(robot?.id)
  const failed = robot !== undefined && needsRobotAttention(robot, branch.commits[0]?.hash, branch.remoteStatus)
  const robotAttention = failed ? (needsReview ? "warning" : "error") : null
  return useMemo(() => getLaneNode(branch, landedIn, robotAttention), [branch, landedIn, robotAttention])
}
