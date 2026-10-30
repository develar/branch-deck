import { toast } from "@/components/ui/toast"
import { formatSessionStatus, formatWatchEnd, stopAirWatch, watchAirSession } from "@/lib/air-launch"
import { getErrorDetails } from "@/lib/errors"
import { TOAST_TIMEOUT_MS } from "@/lib/toast-message"

/**
 * The toast of a session that Start in Air started (spec/branch/start-in-air.spec.md): it follows the status of the
 * session through `watch` in place ("works", "waits for you", "is done") until the session is done, the session is
 * gone or the watch ends. A close of the toast stops the watch.
 */
export function followAirSession(project: string, sessionId: string, agent: string, description: string): void {
  // the session is done, or the user closed the toast: the toast says nothing more
  let settled = false
  const id = toast.add({
    type: "loading",
    title: `Started ${agent} in Air`,
    description,
    timeout: 0,
    onClose: () => {
      if (!settled) {
        settled = true
        stopAirWatch(sessionId)
      }
    },
  })
  const settle = (update: { type: "loading" | "info" | "success" | "warning"; title: string; description: string }) => {
    settled = true
    toast.update(id, { ...update, timeout: TOAST_TIMEOUT_MS })
  }
  watchAirSession(project, sessionId, status => {
    if (settled) {
      return
    }
    const shown = formatSessionStatus(agent, status.activity)
    const update = { type: shown.type, title: shown.title, description: status.title ?? description }
    if (shown.done) {
      settle(update)
      stopAirWatch(sessionId)
    } else {
      toast.update(id, update)
    }
  }).then(
    end => {
      const words = settled ? null : formatWatchEnd(agent, end)
      if (words !== null) {
        settle({ ...words, description: words.description ?? description })
      }
    },
    (error: unknown) => {
      if (!settled) {
        settle({ type: "warning", title: `Cannot follow ${agent} in Air`, description: getErrorDetails(error) })
      }
    },
  )
}
