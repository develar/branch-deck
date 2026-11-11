import { describe, expect, it } from "vite-plus/test"
import type { MenuItem } from "@/lib/menu"
import { frozenMenuGroups } from "@/lib/menu"

/** An item of a menu by its id and label */
function item(id: string, label = id, patch: Partial<MenuItem> = {}): MenuItem {
  return { id, label, onSelect: () => {}, ...patch }
}

/** The labels of the groups, a disabled item marked */
function labels(groups: MenuItem[][]): string[][] {
  return groups.map(group => group.map(entry => `${entry.label}${entry.disabled === true ? " (disabled)" : ""}`))
}

describe("an open menu", () => {
  it("shows each item in its place as it is now, keeps an item that left there disabled and shows a new one at the next open", () => {
    const opened = [[item("merge", "Merge", { disabled: true }), item("push", "Push Branch (3)")], [item("review-changes", "Review Changes")]]
    const now = [[item("fix-conflict", "Fix Conflict…")], [item("merge", "Merge #7"), item("push", "Push Branch (4)")], [item("open-pull-request", "Open Pull Request #7")]]
    expect(labels(frozenMenuGroups(opened, now))).toEqual([["Merge #7", "Push Branch (4)"], ["Review Changes (disabled)"]])
    // what the menu shows as it opens again
    expect(labels(frozenMenuGroups(now, now))).toEqual(labels(now))
  })

  it("shows the item that took the slot of another in its place: Open Pull Request #7 where Create Pull Request… was", () => {
    const create = item("create-pull-request", "Create Pull Request…", { slot: "pull-request", disabled: true })
    const open = item("open-pull-request", "Open Pull Request #7", { slot: "pull-request" })
    const opened = [[item("review-changes", "Review Changes"), create, item("request-review", "Request Review…", { disabled: true })]]
    const now = [[item("review-changes", "Review Changes"), open, item("request-review", "Request Review…")]]
    expect(labels(frozenMenuGroups(opened, now))).toEqual([["Review Changes", "Open Pull Request #7", "Request Review…"]])
  })

  it("freezes an open submenu too, and a submenu that left is a disabled item that opens nothing", () => {
    const copy = (children: MenuItem[]) => item("copy", "Copy", { children })
    const opened = [
      [copy([item("copy-name", "Copy Branch Name"), item("copy-link", "Copy Pull Request Link")])],
      [item("patronus", "Patronus", { children: [item("dry-run", "Dry Run")] })],
    ]
    const now = [[copy([item("copy-full-name", "Copy Full Branch Name"), item("copy-name", "Copy Branch Name")])]]
    const frozen = frozenMenuGroups(opened, now)
    expect(frozen[0]![0]!.children?.map(child => `${child.label}${child.disabled === true ? " (disabled)" : ""}`)).toEqual([
      "Copy Branch Name",
      "Copy Pull Request Link (disabled)",
    ])
    expect(frozen[1]![0]).toMatchObject({ label: "Patronus", disabled: true, children: undefined })
  })

  it("matches the items of one key in the order of the menu, and a running item that left runs no longer", () => {
    const opened = [[item("copy", "Copy"), item("copy", "Copy"), item("restart", "Restart Safe Push", { running: true })]]
    const now = [[item("copy", "Copy", { description: "first" })]]
    expect(frozenMenuGroups(opened, now)[0]).toMatchObject([{ description: "first" }, { disabled: true }, { disabled: true, running: false }])
  })
})
