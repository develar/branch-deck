const formatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "short",
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
  else if (dayDiff === 0) {
    return `Yesterday ${timeFormatter.format(date)}`
  }
  else {
    return formatter.format(date)
  }
}
