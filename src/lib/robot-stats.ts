import type { RobotFailureKind, Share, StatsPeriod } from "@/lib/bindings"

/** The periods of the statistics, the shortest first, as `StatsPeriod` names them */
export const STATS_PERIODS = ["14d", "30d", "10w", "6m"] as const satisfies readonly StatsPeriod[]

/** "30 days", "10 weeks": the days a period counts */
export function formatStatsPeriod(period: StatsPeriod): string {
  switch (period) {
    case "14d":
      return "14 days"
    case "30d":
      return "30 days"
    case "10w":
      return "10 weeks"
    case "6m":
      return "6 months"
  }
}

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

/** "10:00" */
export function formatHour(hour: number): string {
  return `${String(hour).padStart(2, "0")}:00`
}

/**
 * How the user's value compares to everyone else's, where lower is better: "22% more", "8% less", "the same"; `better`
 * says the direction is good. None without both values
 */
export function compareToEveryone(mine: number | null, everyone: number | null): { text: string; better: boolean | null } | null {
  if (mine === null || everyone === null || everyone === 0) {
    return null
  }
  const percent = Math.round(((mine - everyone) / everyone) * 100)
  if (percent === 0) {
    return { text: "the same as everyone else", better: null }
  }
  return { text: `${Math.abs(percent)}% ${percent > 0 ? "more" : "less"} than everyone else`, better: percent < 0 }
}
