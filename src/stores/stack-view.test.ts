// @spec spec/branch/branch-row.spec.md
import { beforeEach, describe, expect, it, vi } from "vitest"
import { layoutStack } from "@/lib/stack-lanes"
import { focusStackPath, hoverStackPath, pinStackPath, setStackPathRows, stackPathStore } from "@/stores/stack-view"

// the push of a hovered button reads the branches; the path doesn't
vi.mock("@/stores/branch-sync", () => ({ getBranch: () => undefined }))

const ROWS = layoutStack("auth", name => ({ auth: ["session", "login"], login: ["oauth"] } as Record<string, string[]>)[name] ?? [])
  .map(({ name, lanes }) => ({ name, lanes }))

/** The rows on the highlighted path */
function path(): string[] {
  return [...stackPathStore.getState().segments.keys()].sort()
}

describe("stack path", () => {
  beforeEach(() => {
    hoverStackPath(null)
    focusStackPath(null)
    pinStackPath("oauth", false)
    pinStackPath("session", false)
    setStackPathRows(ROWS)
  })

  it("highlights the path from the root to the hovered branch, and keeps its state while the branch doesn't change", () => {
    hoverStackPath({ name: "oauth", push: false })
    expect(path()).toEqual(["auth", "login", "oauth", "session"])
    const state = stackPathStore.getState()
    hoverStackPath({ name: "oauth", push: false })
    expect(stackPathStore.getState()).toBe(state)
  })

  it("prefers the pointer to the keyboard focus, and the focus to an open Safe Push form", () => {
    pinStackPath("session", true)
    expect(path()).toEqual(["auth", "session"])
    focusStackPath({ name: "oauth", push: false })
    expect(path()).toEqual(["auth", "login", "oauth", "session"])
    hoverStackPath({ name: "session", push: false })
    hoverStackPath(null)
    focusStackPath(null)
    // the form is still open: its path comes back
    expect(path()).toEqual(["auth", "session"])
    pinStackPath("oauth", false)
    expect(path()).toEqual(["auth", "session"])
    pinStackPath("session", false)
    expect(path()).toEqual([])
  })

  it("drops the path of a branch that left the rows", () => {
    hoverStackPath({ name: "oauth", push: false })
    setStackPathRows(ROWS.filter(row => row.name !== "oauth"))
    expect(path()).toEqual([])
    expect(stackPathStore.getState().hovered).toBeNull()
  })
})
