import { describe, expect, it } from "vite-plus/test"
import type { SettingsPageId } from "@/lib/settings-pages"
import { resolveSettingsPage, resolveSettingsScope } from "@/lib/settings-window"

describe("resolveSettingsPage", () => {
  it("shows the page an open names", () => {
    expect(resolveSettingsPage("general", { page: "notifications" })).toBe("notifications")
    expect(resolveSettingsPage("accounts", { page: "branches", scope: "repository", focus: "branchPrefix" })).toBe("branches")
  })

  it("keeps the current page for an open without one", () => {
    expect(resolveSettingsPage("appearance", {})).toBe("appearance")
    expect(resolveSettingsPage("accounts", { focus: "branchPrefix" })).toBe("accounts")
    expect(resolveSettingsPage("notifications", null)).toBe("notifications")
  })

  it("shows the page that has the settings of a page of before, and General for a page the sidebar doesn't know", () => {
    expect(resolveSettingsPage("general", { page: "repository" as SettingsPageId })).toBe("branches")
    expect(resolveSettingsPage("general", { page: "companions" as SettingsPageId })).toBe("landing")
    expect(resolveSettingsPage("general", { page: "github" as SettingsPageId })).toBe("accounts")
    expect(resolveSettingsPage("youtrack" as SettingsPageId, {})).toBe("accounts")
    expect(resolveSettingsPage("general", { page: "window" as SettingsPageId })).toBe("general")
  })
})

describe("resolveSettingsScope", () => {
  it("shows all repositories unless an open names the repository, never the scope of an open before", () => {
    expect(resolveSettingsScope(null)).toBe("all")
    expect(resolveSettingsScope({ page: "agents" })).toBe("all")
    expect(resolveSettingsScope({ page: "agents", scope: "repository" })).toBe("repository")
  })
})
