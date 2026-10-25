import { useEffect } from "react"
import type { PatronusRobotDetails } from "@/lib/bindings"
import { showRobotDetails } from "@/model/demand"
import { useRobotDetailsFact } from "@/stores/robot-details"

/**
 * The checks, the problems and the profile of a robot while `open` (its popover, the robots of the landing overview):
 * the window demands them of the session for as long (`showRobotDetails`), which reads them at once, again when the
 * robot's status changes and, of a running robot with the estimate of the CI, every minute while they are shown
 * (`robot-details.ts`). Null while the session has none (not answered yet, or Patronus can't answer: the popover shows
 * the list data then). Nothing is asked while no details are open.
 */
export function useRobotDetails(robotId: string, open: boolean): PatronusRobotDetails | null {
  useEffect(() => (open ? showRobotDetails(robotId) : undefined), [robotId, open])
  const details = useRobotDetailsFact(robotId)
  return open ? details : null
}
