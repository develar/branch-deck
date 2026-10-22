// @spec spec/repository/command-palette.spec.md

import { ListTreeIcon } from "lucide-react"
import { describe, expect, it, vi } from "vite-plus/test"
import { runAppCommand } from "@/lib/app-commands"
import { paletteCommands } from "@/lib/command-palette"
import type { PaletteSources } from "@/lib/command-palette"

vi.mock("@/lib/app-commands", () => ({ runAppCommand: vi.fn() }))

function sources(overrides: Partial<PaletteSources> = {}): PaletteSources {
  return {
    views: [{ view: "branches", label: "Branches", icon: ListTreeIcon }],
    showView: vi.fn(),
    menu: { sync: { canSync: true, reason: null }, autoSyncOnFocus: true, updates: false },
    branches: ["auth", "IJPL-1"],
    toggleActivityPanel: vi.fn(),
    openActivityWindow: vi.fn(),
    goToBranch: vi.fn(),
    ...overrides,
  }
}

describe("paletteCommands", () => {
  it("lists the views, Activity, the commands of the app's menu and the branches, by namespaced ids", () => {
    expect(paletteCommands(sources()).map(command => [command.id, command.label, command.group])).toEqual([
      ["view:branches", "Branches", "Views"],
      ["activity:panel", "Activity Panel", "Activity"],
      ["activity:window", "Activity Window", "Activity"],
      ["app:sync_branches", "Sync Branches", "Commands"],
      ["app:update_project", "Update Project", "Commands"],
      ["app:open_settings", "Settings…", "Commands"],
      ["app:install_agent_cli", "Install Agent CLI…", "Commands"],
      ["branch:auth", "Go to auth", "Branches"],
      ["branch:IJPL-1", "Go to IJPL-1", "Branches"],
    ])
  })

  it("hints only the keys of the window, disables Sync with its reason, offers updates where the build updates itself", () => {
    const commands = paletteCommands(sources({ menu: { sync: { canSync: false, reason: "Set the branch prefix first" }, autoSyncOnFocus: true, updates: true } }))
    const byId = new Map(commands.map(command => [command.id, command]))
    expect(byId.get("app:sync_branches")).toMatchObject({ kbds: ["mod", "r"], disabled: true, description: "Set the branch prefix first" })
    expect(byId.get("app:open_settings")?.kbds).toEqual(["mod", ","])
    expect(byId.has("app:check_for_updates")).toBe(true)
    expect(commands.filter(command => command.kbds !== undefined).map(command => command.id)).toEqual(["app:sync_branches", "app:update_project", "app:open_settings"])
  })

  it("keeps one entry per id, the first source's", () => {
    const commands = paletteCommands(
      sources({
        views: [
          { view: "branches", label: "Branches", icon: ListTreeIcon },
          { view: "branches", label: "Branches again", icon: ListTreeIcon },
        ],
        branches: ["auth", "auth"],
      }),
    )
    expect(commands.filter(command => command.id === "view:branches").map(command => command.label)).toEqual(["Branches"])
    expect(commands.filter(command => command.id === "branch:auth")).toHaveLength(1)
  })

  it("runs what its source runs", () => {
    const given = sources()
    const byId = new Map(paletteCommands(given).map(command => [command.id, command]))
    byId.get("view:branches")!.run()
    expect(given.showView).toHaveBeenCalledWith(given.views[0])
    byId.get("activity:panel")!.run()
    expect(given.toggleActivityPanel).toHaveBeenCalled()
    byId.get("activity:window")!.run()
    expect(given.openActivityWindow).toHaveBeenCalled()
    byId.get("branch:IJPL-1")!.run()
    expect(given.goToBranch).toHaveBeenCalledWith("IJPL-1")
    byId.get("app:update_project")!.run()
    expect(runAppCommand).toHaveBeenCalledWith("update_project")
  })
})
