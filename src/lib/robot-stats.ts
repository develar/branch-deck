import type { RobotFailureKind, Share } from "@/lib/bindings"

/** What failed, as the statistics name a kind (`RobotFailureKind`) */
export function formatFailureKind(kind: RobotFailureKind): string {
  switch (kind) {
    case "checks":
      return "Checks failed"
    case "infrastructure":
      return "Builds never started"
    case "massive":
      return "Massive failure"
    case "patronus":
      return "Patronus error"
    case "unknown":
      return "Unknown"
  }
}

/** "1.4", one decimal; "–" without a value */
export function formatRatio(value: number | null): string {
  return value === null ? "–" : value.toFixed(1)
}

/** "71%" of a share; "–" of none */
export function formatShare(share: Share | null): string {
  return share === null || share.total === 0 ? "–" : `${Math.round((share.count / share.total) * 100)}%`
}

/** "12 of 17" */
export function formatShareCount(share: Share): string {
  return `${share.count} of ${share.total}`
}

const weekFormat = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" })

/** "Oct 19", the local Monday of a week (Unix seconds) */
export function formatWeek(start: number): string {
  return weekFormat.format(new Date(start * 1000))
}
