import { useEffect, useState } from "react"
import type { PatronusRobotDetails } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { patronusRequest } from "@/stores/patronus"

/** A popover that opens and closes at once asks nothing */
export const DETAILS_DELAY_MS = 300

/**
 * The checks, the problems and the profile of a robot, asked when `enabled` (its popover is open) has lasted a moment.
 * The backend caches them: a finished robot for the session, a running one for a minute. Null while unknown or when
 * Patronus can't answer (the popover shows the list data then).
 */
export function usePatronusRobotDetails(robotId: string, status: string, enabled: boolean): PatronusRobotDetails | null {
  const [details, setDetails] = useState<{ key: string, value: PatronusRobotDetails } | null>(null)
  // a new status (a poll) asks again: the checks moved on
  const key = `${robotId}\n${status}`
  useEffect(() => {
    if (!enabled) {
      return
    }
    let current = true
    const timer = setTimeout(() => {
      commands.getPatronusRobotDetails({ ...patronusRequest(), robotId }).then((result) => {
        if (current && result.status === "ok") {
          setDetails({ key, value: result.data })
        }
      }, () => {
        // the details are an extra: the popover shows the list data
      })
    }, DETAILS_DELAY_MS)
    return () => {
      current = false
      clearTimeout(timer)
    }
  }, [enabled, robotId, key])
  return details?.key === key ? details.value : null
}
