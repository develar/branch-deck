const formatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
})

const timeFormatter = new Intl.DateTimeFormat(undefined, {
  timeStyle: "short",
})

const dayFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
})

const dateFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
})

const msPerDay = 24 * 60 * 60 * 1000

/** Whole calendar days from the day of `date` to the day of `now`: 0 today, 1 yesterday */
function daysBefore(date: Date, now: Date): number {
  return Math.round((new Date(now).setHours(0, 0, 0, 0) - new Date(date).setHours(0, 0, 0, 0)) / msPerDay)
}

/** "Today 2:32 PM", "Yesterday 9:10 AM", "Oct 3, 2026, 2:32 PM": the time of a commit on its meta line */
export function formatTimestamp(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000)
  const dayDiff = daysBefore(date, new Date())
  if (dayDiff === 0) {
    return `Today ${timeFormatter.format(date)}`
  } else if (dayDiff === 1) {
    return `Yesterday ${timeFormatter.format(date)}`
  } else {
    return formatter.format(date)
  }
}

/**
 * "Today 2:32 PM", "Yesterday", "Oct 3", "Oct 3, 2025": the time of a commit in a one-line list (the commits of a pull
 * request, the stack island), as short as its day allows; the meta line and its popover say it whole (`formatTimestamp`)
 */
export function formatCompactTimestamp(unixSeconds: number, now: Date = new Date()): string {
  const date = new Date(unixSeconds * 1000)
  const dayDiff = daysBefore(date, now)
  if (dayDiff === 0) {
    return `Today ${timeFormatter.format(date)}`
  }
  if (dayDiff === 1) {
    return "Yesterday"
  }
  return date.getFullYear() === now.getFullYear() ? dayFormatter.format(date) : dateFormatter.format(date)
}

/** "just now", "5 minutes ago", "yesterday": how long before `now` the `timestamp` (both in milliseconds) was */
export function formatRelativeTime(timestamp: number, now: number = Date.now()): string {
  const diff = now - timestamp

  const seconds = Math.floor(diff / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)

  if (seconds < 60) {
    return "just now"
  }

  if (minutes < 60) {
    return minutes === 1 ? "1 minute ago" : `${minutes} minutes ago`
  }

  if (hours < 24) {
    return hours === 1 ? "1 hour ago" : `${hours} hours ago`
  }

  if (days < 30) {
    return days === 1 ? "yesterday" : `${days} days ago`
  }

  const months = Math.floor(days / 30)
  if (months < 12) {
    return months === 1 ? "1 month ago" : `${months} months ago`
  }

  const years = Math.floor(months / 12)
  return years === 1 ? "1 year ago" : `${years} years ago`
}

/**
 * Whether the time `later` says more than `earlier` (both in milliseconds) in relative words: when `later` came,
 * `earlier` read no longer "just now". Decided at `later`, so it holds while both age; their texts compared now would
 * flip at every edge of a bucket ("1 hour ago" beside "2 hours ago", then the same again)
 */
export function readsLater(later: number, earlier: number): boolean {
  return formatRelativeTime(earlier, later) !== formatRelativeTime(later, later)
}
