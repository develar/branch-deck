import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import { formatRelativeTime } from "@/lib/time"

/**
 * "just now", "5 minutes ago": text that ages while it is shown. Only this text re-renders on the tick, not the
 * component around it.
 */
export function RelativeTime({
  time,
  prefix = "",
}: {
  /** Milliseconds since the epoch */
  time: number
  /** Words before the time in the same text node, "Last synced ": one run of text, measured as one */
  prefix?: string
}) {
  const now = useNow(RELATIVE_TIME_TICK_MS)
  return prefix + formatRelativeTime(time, now)
}
