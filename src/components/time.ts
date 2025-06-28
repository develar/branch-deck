import { format, isToday, isYesterday } from "date-fns"

const formatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "short",
  timeStyle: "short",
})

export function formatTimestamp(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000)
  if (isToday(date)) {
    return `Today ${format(date, "HH:mm")}`
  }
  else if (isYesterday(date)) {
    return `Yesterday ${format(date, "HH:mm")}`
  }
  else {
    return formatter.format(date)
  }
}