import { useEffect, useState } from "react"
import type { PatronusRobotDetails } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { markRobotFailing, patronusRequest, rememberRobotProfile } from "@/stores/patronus"
import { holdRobotProgress, setRobotProgress } from "@/stores/robot-progress"

/** A popover that opens and closes at once asks nothing */
export const DETAILS_DELAY_MS = 300
/**
 * The details of a running robot are asked again this often while they are open: the backend caches them and the
 * progress of each build for a minute, as Patronus caches the progress, so asking more often would read the same answer
 */
export const RUNNING_DETAILS_REFRESH_MS = 61_000

/**
 * The checks, the problems and the profile of a robot, asked when `enabled` (its popover is open) has lasted a moment;
 * of a running robot also the estimate of the CI for its running checks (`progress`), asked again every minute while
 * open and kept for the slot of its row (`robot-progress.ts`) until it closes; one that will fail is marked failing
 * (`markRobotFailing`). The backend caches them: a finished robot for the session, a running one for a minute. Null
 * while unknown or when Patronus can't answer (the popover shows the list data then). Nothing is asked while no details
 * are open.
 */
export function usePatronusRobotDetails(robotId: string, status: string, enabled: boolean): PatronusRobotDetails | null {
  const [details, setDetails] = useState<{ key: string; value: PatronusRobotDetails } | null>(null)
  // a new status (a poll) asks again: the checks moved on
  const key = `${robotId}\n${status}`
  useEffect(() => {
    if (!enabled) {
      return
    }
    let current = true
    const running = status === "running"
    const release = running ? holdRobotProgress(robotId) : null
    const ask = () => {
      commands.getPatronusRobotDetails({ ...patronusRequest(), robotId }).then(
        result => {
          if (current && result.status === "ok") {
            setDetails({ key, value: result.data })
            // the details name the merge profile, which the estimate of the robot's time keeps
            rememberRobotProfile(result.data.robot)
            if (running) {
              setRobotProgress(robotId, result.data.progress)
              // the chips know it before the next poll
              if (result.data.failing) {
                markRobotFailing(robotId)
              }
            }
          }
        },
        () => {
          // the details are an extra: the popover shows the list data
        },
      )
    }
    let refresh: ReturnType<typeof setInterval> | undefined
    const timer = setTimeout(() => {
      ask()
      if (running) {
        refresh = setInterval(ask, RUNNING_DETAILS_REFRESH_MS)
      }
    }, DETAILS_DELAY_MS)
    return () => {
      current = false
      clearTimeout(timer)
      clearInterval(refresh)
      release?.()
    }
  }, [enabled, robotId, status, key])
  return details?.key === key ? details.value : null
}
