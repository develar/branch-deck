import type { ReactNode } from "react"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { formatPushMode } from "@/lib/patronus"
import { useLandingSpan } from "@/stores/patronus"

/**
 * "lands with oauth" in the Status cell of a branch that a running robot of a branch built on it lands
 * (`useLandingSpan`): the robot pushes the commits of every branch the branch is built on. `fallback` ("on master" of a
 * branch built elsewhere) while none runs: the landing robot says more about the branch now.
 */
export function LandingSpanLabel({ name, fallback }: { name: string; fallback: ReactNode }) {
  const landing = useLandingSpan(name)
  if (landing === null) {
    return fallback
  }
  return (
    <Tooltip>
      <TooltipTrigger render={<span className="shrink-0 text-xs text-muted-foreground" data-testid="landing-span" />}>{`lands with ${landing.branch}`}</TooltipTrigger>
      <TooltipContent>{`A ${formatPushMode(landing.robot.pushMode)} of ${landing.branch} lands this branch with it`}</TooltipContent>
    </Tooltip>
  )
}
