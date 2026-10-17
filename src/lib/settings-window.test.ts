import { describe, expect, it } from "vite-plus/test"
import type { SettingsPageId } from "@/lib/settings-pages"
import { resolveSettingsPage } from "@/lib/settings-window"

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

  it("shows General for a page the sidebar doesn't know", () => {
    expect(resolveSettingsPage("github", { page: "repository" })).toBe("repository")
    expect(resolveSettingsPage("github", { page: "window" as SettingsPageId })).toBe("general")
  })
})
