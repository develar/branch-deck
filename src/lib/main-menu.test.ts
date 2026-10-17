import { describe, expect, it } from "vite-plus/test"
import { mainMenus, matchMainMenuKey, matchMainMenuOpenKey } from "@/lib/main-menu"

const ready = { sync: { canSync: true, reason: null }, autoSyncOnFocus: true, updates: false }

function key(init: Partial<KeyboardEvent>) {
  return { key: "", code: "", ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...init }
}

describe("mainMenus", () => {
  it("holds the items of the native menu, File, View and Help", () => {
    expect(mainMenus(ready).map(menu => [menu.label, menu.groups.flat().map(item => item.label)])).toEqual([
      ["File", ["Sync Branches", "Update Project", "Auto-sync on Focus", "Settings…"]],
      ["View", ["Actual Size", "Zoom In", "Zoom Out", "Theme…"]],
      ["Help", ["Getting Started", "Install Agent CLI…", "GitHub Repository"]],
    ])
  })

  it("disables Sync with the reason of the Sync button, checks Auto-sync, offers updates in a release build", () => {
    const menus = mainMenus({ sync: { canSync: false, reason: "Set the branch prefix first" }, autoSyncOnFocus: false, updates: true })
    const items = menus.flatMap(menu => menu.groups.flat())
    expect(items.find(item => item.id === "sync_branches")).toMatchObject({ disabled: true, description: "Set the branch prefix first" })
    expect(items.find(item => item.id === "auto_sync_on_focus")?.checked).toBe(false)
    expect(items.some(item => item.id === "check_for_updates")).toBe(true)
  })
})

describe("matchMainMenuKey", () => {
  it("maps the accelerators of the native menu to their commands", () => {
    expect(matchMainMenuKey(key({ code: "KeyR", ctrlKey: true }))).toEqual({ command: "sync-branches" })
    // the physical key: a Cyrillic layout's Ctrl+R
    expect(matchMainMenuKey(key({ code: "KeyR", key: "к", ctrlKey: true }))).toEqual({ command: "sync-branches" })
    expect(matchMainMenuKey(key({ code: "KeyT", ctrlKey: true }))).toEqual({ command: "update-project" })
    expect(matchMainMenuKey(key({ code: "Comma", ctrlKey: true }))).toEqual({ command: "open_settings" })
    expect(matchMainMenuKey(key({ code: "Equal", ctrlKey: true }))).toEqual({ command: "zoom", payload: "in" })
    expect(matchMainMenuKey(key({ code: "Minus", ctrlKey: true }))).toEqual({ command: "zoom", payload: "out" })
    expect(matchMainMenuKey(key({ code: "Digit0", ctrlKey: true }))).toEqual({ command: "zoom", payload: "reset" })
  })

  it("leaves other keys alone, Ctrl+Shift+= too, which useZoom takes", () => {
    expect(matchMainMenuKey(key({ code: "KeyR" }))).toBeNull()
    expect(matchMainMenuKey(key({ code: "KeyR", metaKey: true }))).toBeNull()
    expect(matchMainMenuKey(key({ code: "Equal", key: "+", ctrlKey: true, shiftKey: true }))).toBeNull()
    expect(matchMainMenuKey(key({ code: "KeyR", ctrlKey: true, altKey: true }))).toBeNull()
  })
})

describe("matchMainMenuOpenKey", () => {
  it("opens File on Alt+\\ and F10, a menu on Alt with its mnemonic, by the physical key", () => {
    expect(matchMainMenuOpenKey(key({ code: "Backslash", altKey: true }))).toBe("file")
    expect(matchMainMenuOpenKey(key({ key: "F10" }))).toBe("file")
    expect(matchMainMenuOpenKey(key({ code: "KeyV", key: "м", altKey: true }))).toBe("view")
    expect(matchMainMenuOpenKey(key({ code: "KeyH", altKey: true }))).toBe("help")
    expect(matchMainMenuOpenKey(key({ code: "KeyF", altKey: true, ctrlKey: true }))).toBeNull()
    expect(matchMainMenuOpenKey(key({ code: "KeyQ", altKey: true }))).toBeNull()
  })
})
