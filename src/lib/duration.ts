const MINUTE = 60
const HOUR = 3600
const DAY = 86_400

/** A total of robot time: "0 min", "45 min", "6 h", "3 d 4 h" (days from 48 hours, so "36 h" stays one figure) */
export function formatTotalDuration(seconds: number): string {
  if (seconds <= 0) {
    return "0 min"
  }
  if (seconds < HOUR) {
    return `${Math.max(1, Math.round(seconds / MINUTE))} min`
  }
  if (seconds < 2 * DAY) {
    return `${Math.round(seconds / HOUR)} h`
  }
  const days = Math.floor(seconds / DAY)
  const hours = Math.round((seconds % DAY) / HOUR)
  return hours === 0 ? `${days} d` : `${days} d ${hours} h`
}
