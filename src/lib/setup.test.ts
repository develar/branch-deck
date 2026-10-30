// @spec spec/repository/setup-attention.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { SetupEntry } from "@/lib/bindings"
import { describeSetupItem, settingsButtonLabel, setupSkipUntil } from "@/lib/setup"

function entry(item: SetupEntry["item"], skippable = true): SetupEntry {
  return { key: item.kind, item, skippable }
}

describe("setup dot", () => {
  it("names each item, its action and its skip", () => {
    expect(describeSetupItem(entry({ kind: "appUpdate", version: "1.2.0" }))).toEqual({
      title: "Branch Deck 1.2.0 is ready to install",
      action: "Install and Restart",
      skip: "Skip This Version",
    })
    expect(describeSetupItem(entry({ kind: "appUpdateCheckFailed", message: "Network unavailable" }))).toEqual({
      title: "Cannot check for updates",
      detail: "Network unavailable",
      action: "Check Again",
      skip: "Remind Tomorrow",
    })
    expect(describeSetupItem(entry({ kind: "toolUpdate", tool: "youtrack", version: "0.8.0" }))).toEqual({
      title: "youtrack 0.8.0 is out",
      action: "Update",
      skip: "Skip This Version",
    })
    expect(describeSetupItem(entry({ kind: "skillUpdate", tool: "teamcity", version: "1.4.0" })).title).toBe("The teamcity agent skill 1.4.0 is ready to install")
    expect(describeSetupItem(entry({ kind: "signInExpiring", tool: "teamcity", server: "https://ci.example", expiresAt: Date.UTC(2024, 0, 5, 12) / 1000 }))).toEqual({
      title: "The teamcity sign-in to ci.example expires Jan 5",
      action: "Sign In",
      skip: "Remind Tomorrow",
    })
    expect(describeSetupItem(entry({ kind: "signInExpired", tool: "teamcity", server: "https://ci.example" }, false))).toEqual({
      title: "The teamcity sign-in to ci.example expired",
      action: "Sign In",
      skip: null,
    })
    expect(describeSetupItem(entry({ kind: "youTrackSignIn" }, false)).skip).toBeNull()
  })

  it("skips an update for good and a reminder for a day, and counts the items in the name of Settings", () => {
    expect(setupSkipUntil(entry({ kind: "toolUpdate", tool: "teamcity", version: "1.7.0" }), 100)).toBeNull()
    expect(setupSkipUntil(entry({ kind: "appUpdateCheckFailed", message: "Network unavailable" }), 100)).toBe(100 + 86_400)
    expect(setupSkipUntil(entry({ kind: "signInExpiring", tool: "teamcity", server: "https://ci.example", expiresAt: 500_000 }), 100)).toBe(100 + 86_400)
    expect(settingsButtonLabel(0)).toBe("Settings")
    expect(settingsButtonLabel(2)).toBe("Settings · 2 to do")
  })
})
