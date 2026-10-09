const formatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
})

const timeFormatter = new Intl.DateTimeFormat(undefined, {
  timeStyle: "short",
})

const msPerDay = 24 * 60 * 60 * 1000

export function formatTimestamp(unixSeconds: number): string {
  const now = new Date()
  const dayDiff = Math.floor((now.setHours(0, 0, 0, 0) - new Date(unixSeconds * 1000).setHours(0, 0, 0, 0)) / msPerDay)

  const date = new Date(unixSeconds * 1000)
  if (dayDiff === 0) {
    return `Today ${timeFormatter.format(date)}`
  }
  else if (dayDiff === 1) {
    return `Yesterday ${timeFormatter.format(date)}`
  }
  else {
    return formatter.format(date)
  }
}

export function formatRelativeTime(timestamp: number): string {
  const now = Date.now()
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
