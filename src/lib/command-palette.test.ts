// @spec spec/repository/command-palette.spec.md

import { ListTreeIcon, ShieldCheckIcon } from "lucide-react"
import { describe, expect, it, vi } from "vite-plus/test"
import { runAppCommand } from "@/lib/app-commands"
import { paletteCommands, paletteGroups } from "@/lib/command-palette"
import type { PaletteSources } from "@/lib/command-palette"
import type { MenuItemGroups } from "@/lib/menu"

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
    branchMenu: null,
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

  it("offers the actions of the branch at hand as its menu has them, an item of a submenu named with it, disabled with the reason", () => {
    const review = vi.fn()
    const dryRun = vi.fn()
    const groups: MenuItemGroups = [
      [
        { id: "safe-push", label: "Safe Push", icon: ShieldCheckIcon, kbds: ["s"], disabled: true, description: "Waits for the answer of Patronus", onSelect: () => {} },
        { id: "patronus", label: "Patronus", onSelect: () => {}, children: [{ id: "dry-run", label: "Dry Run", onSelect: dryRun }] },
      ],
      [{ id: "review-changes", label: "Review Changes", kbds: ["d"], description: "The diff of the branch", onSelect: review }],
      [{ id: "stack-on", label: "Stack On…", onSelect: () => {}, children: [{ label: "docs", onSelect: () => {} }] }],
    ]
    const commands = paletteCommands(sources({ branchMenu: { name: "auth", groups } })).filter(command => command.group === "Branch")
    expect(commands.map(command => [command.id, command.label, command.disabled ?? false, command.description ?? null])).toEqual([
      ["action:auth:safe-push", "Safe Push", true, "Waits for the answer of Patronus"],
      ["action:auth:patronus/dry-run", "Patronus › Dry Run", false, null],
      ["action:auth:review-changes", "Review Changes", false, null],
      ["action:auth:stack-on/docs", "Stack On › docs", false, null],
    ])
    // the keys of the row are no keys of the window
    expect(commands.some(command => command.kbds !== undefined)).toBe(false)
    commands[1]!.run()
    commands[2]!.run()
    expect([dryRun.mock.calls.length, review.mock.calls.length]).toEqual([1, 1])
    // between the commands of the app and the branches
    expect([...new Set(paletteCommands(sources({ branchMenu: { name: "auth", groups } })).map(command => command.group))]).toEqual([
      "Views",
      "Activity",
      "Commands",
      "Branch",
      "Branches",
    ])
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

describe("paletteGroups", () => {
  // as the filter of the list compares: case aside
  const startsWith = (value: string, query: string) => value.toLowerCase().startsWith(query.toLowerCase())
  const menu: MenuItemGroups = [
    [
      { id: "push", label: "Push Stack (2 branches)", onSelect: () => {} },
      { id: "push-branch-only", label: "Push session Only", onSelect: () => {} },
    ],
    [{ id: "stack-on", label: "Stack On…", onSelect: () => {}, children: [{ label: "auth", onSelect: () => {} }] }],
  ]
  const commands = paletteCommands(sources({ branches: ["auth", "session"], branchMenu: { name: "session", groups: menu } }))
  const listed = (text: string) => paletteGroups(commands, text, startsWith).map(group => [group.value, group.items.map(command => command.label)])

  it("lists the groups in their order without a text, a command and its group that start with the text first", () => {
    expect(listed("").map(([group]) => group)).toEqual(["Views", "Activity", "Commands", "Branch", "Branches"])
    // the name of a branch goes to it before the action of the branch at hand that names it, which Enter would run
    expect(listed("session").slice(0, 2)).toEqual([
      ["Branches", ["Go to session", "Go to auth"]],
      ["Views", ["Branches"]],
    ])
    expect(listed("auth")[0]).toEqual(["Branches", ["Go to auth", "Go to session"]])
    // the words of an action find it first, the commands of the app before the actions where both start with them
    expect(listed("push")[0]).toEqual(["Branch", ["Push Stack (2 branches)", "Push session Only", "Stack On › auth"]])
    expect(listed("s").slice(0, 2)).toEqual([
      ["Commands", ["Sync Branches", "Settings…", "Update Project", "Install Agent CLI…"]],
      ["Branch", ["Stack On › auth", "Push Stack (2 branches)", "Push session Only"]],
    ])
    // a Go to found by its label keeps its place among the branches
    expect(listed("go to")[0]).toEqual(["Branches", ["Go to auth", "Go to session"]])
  })
})
