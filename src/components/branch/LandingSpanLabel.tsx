import type { ReactNode } from "react"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LandedWithChip } from "@/components/branch/LandedChip"
import { formatLandingSubject, formatPushMode } from "@/lib/patronus"
import { useBranchLanded } from "@/stores/landed"
import { useLandedBy, useLandingSpan } from "@/stores/patronus"

/**
 * "lands with oauth" in the Status cell of a branch that a running robot of a branch built on it lands
 * (`useLandingSpan`): the robot pushes the commits of every branch the branch is built on; "lands with the whole air
 * stack" on every branch of a stack whose robot of the whole stack runs (on its stack branch). Once that robot merged,
 * "Landed with oauth" (`useLandedBy`, `LandedWithChip`) until the fetched baseline has the commits, which the Landed
 * chip beside it shows then. `fallback` ("on master" of a branch built elsewhere) while none runs: the landing robot
 * says more about the branch now.
 */
export function LandingSpanLabel({ name, fallback }: { name: string; fallback: ReactNode }) {
  const landing = useLandingSpan(name)
  const landedBy = useLandedBy(name)
  const landed = useBranchLanded(name)
  if (landedBy !== null) {
    return landed === null ? <LandedWithChip landed={landedBy} /> : null
  }
  if (landing === null) {
    return fallback
  }
  const subject = formatLandingSubject(landing)
  return (
    <Tooltip>
      <TooltipTrigger render={<span className="shrink-0 text-xs text-muted-foreground" data-testid="landing-span" />}>{`lands with ${subject}`}</TooltipTrigger>
      <TooltipContent>{`A ${formatPushMode(landing.robot.pushMode)} of ${subject} lands this branch with it`}</TooltipContent>
    </Tooltip>
  )
}
