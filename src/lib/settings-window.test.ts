import { describe, expect, it } from "vite-plus/test"
import { resolveSettingsPage, shownSettingsPages } from "@/lib/settings-window"

describe("resolveSettingsPage", () => {
  it("shows the page an open names", () => {
    expect(resolveSettingsPage("general", { page: "notifications" })).toBe("notifications")
    expect(resolveSettingsPage("github", { page: "general", focus: "branchPrefix" })).toBe("general")
  })

  it("keeps the current page for an open without one", () => {
    expect(resolveSettingsPage("appearance", {})).toBe("appearance")
    expect(resolveSettingsPage("github", { focus: "branchPrefix" })).toBe("github")
    expect(resolveSettingsPage("notifications", null)).toBe("notifications")
  })

  it("shows General for a page the sidebar doesn't show", () => {
    expect(shownSettingsPages()).toEqual(["general", "appearance", "notifications", "github"])
    expect(resolveSettingsPage("repository", null)).toBe("general")
    expect(resolveSettingsPage("github", { page: "repository" })).toBe("general")
  })
})
