import type { SetupEntry } from "@/lib/bindings"
import { hostOf } from "@/lib/cli-tools"

/*
 * The words of the Setup dot (spec/repository/setup-attention.spec.md). Which items there are is Rust's
 * (`get_setup_items`); this names each and its action.
 */

export interface SetupItemWords {
  readonly title: string
  readonly detail?: string
  /** The button of its action */
  readonly action: string
  /** The skip of a skippable item: an update for good, a reminder for a day; null for one that can't be skipped */
  readonly skip: string | null
}

const DAY_SECONDS = 24 * 60 * 60
const DATE = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" })

export function describeSetupItem(entry: SetupEntry): SetupItemWords {
  const skip = (label: string) => (entry.skippable ? label : null)
  const item = entry.item
  switch (item.kind) {
    case "appUpdate":
      return { title: `Branch Deck ${item.version} is ready to install`, action: "Install and Restart", skip: skip("Skip This Version") }
    case "appUpdateCheckFailed":
      return { title: "Cannot check for updates", detail: item.message, action: "Check Again", skip: skip("Remind Tomorrow") }
    case "toolUpdate":
      return { title: `${item.tool} ${item.version} is out`, action: "Update", skip: skip("Skip This Version") }
    case "skillUpdate":
      return { title: `The ${item.tool} agent skill ${item.version} is ready to install`, action: "Update Skill", skip: skip("Skip This Version") }
    case "signInExpiring":
      return {
        title: `The ${item.tool} sign-in to ${hostOf(item.server)} expires ${DATE.format(new Date(item.expiresAt * 1000))}`,
        action: "Sign In",
        skip: skip("Remind Tomorrow"),
      }
    case "signInExpired":
      return { title: `The ${item.tool} sign-in to ${hostOf(item.server)} expired`, action: "Sign In", skip: null }
    case "youTrackSignIn":
      return { title: "The YouTrack sign-in stopped working", action: "Open YouTrack Settings", skip: null }
  }
}

/** Until when a skip of `entry` holds: an update for good (another version is another item), a reminder a day */
export function setupSkipUntil(entry: SetupEntry, nowSeconds: number): number | null {
  return entry.item.kind === "signInExpiring" || entry.item.kind === "appUpdateCheckFailed" ? nowSeconds + DAY_SECONDS : null
}

/** The accessible name of the Settings button with the dot: "Settings · 2 to do" */
export function settingsButtonLabel(count: number): string {
  return count === 0 ? "Settings" : `Settings · ${count} to do`
}
