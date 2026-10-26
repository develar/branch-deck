// @spec spec/branch/branch-row.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { layoutStack } from "@/lib/stack-lanes"
import { focusStackPath, hoverStackPath, pinStackPath, setStackPathRows, stackPathStore } from "@/stores/stack-view"

// the push of a hovered button reads the branches and their push orders; the path doesn't
const table = vi.hoisted(() => {
  let state: { branches: Record<string, unknown>; pushes: Record<string, unknown> } = { branches: {}, pushes: {} }
  const listeners = new Set<(next: typeof state, previous: typeof state) => void>()
  return {
    getState: () => state,
    setState: (next: typeof state) => {
      const previous = state
      state = next
      listeners.forEach(listener => listener(state, previous))
    },
    subscribe: (listener: (next: typeof state, previous: typeof state) => void) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
})
vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: table,
  getBranch: (name: string) => table.getState().branches[name],
  getPushOrders: (name: string) => table.getState().pushes[name] ?? { stack: [name], action: [name], button: [], children: null },
}))

const ROWS = layoutStack("auth", name => ({ auth: ["session", "login"], login: ["oauth"] } as Record<string, string[]>)[name] ?? []).map(({ name, lanes }) => ({ name, lanes }))

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

  it("prefers the pointer to the keyboard focus, and the focus to an open Safe Push popover", () => {
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

  it("circles what the hovered push sends, the children of a branch with nothing of its own to send, and follows a push", () => {
    const pushed = () => [...stackPathStore.getState().pushNodes]
    table.setState({ branches: { auth: { remoteStatus: { action: { kind: "create" } } } }, pushes: {} })
    hoverStackPath({ name: "auth", push: true })
    expect(pushed()).toEqual(["auth"])
    // pushed while the pointer rests on its button, which is Push with Children now: the circles follow
    const children = { order: ["session", "login", "oauth"], askFirst: [], stackBranch: null, offers: true }
    table.setState({
      branches: { auth: { remoteStatus: { action: { kind: "upToDate" } } } },
      pushes: { auth: { stack: ["auth"], action: ["auth"], button: ["auth"], children } },
    })
    expect(pushed()).toEqual(["session", "login", "oauth"])
    hoverStackPath(null)
    expect(pushed()).toEqual([])
  })

  it("drops the path of a branch that left the rows", () => {
    hoverStackPath({ name: "oauth", push: false })
    setStackPathRows(ROWS.filter(row => row.name !== "oauth"))
    expect(path()).toEqual([])
    expect(stackPathStore.getState().hovered).toBeNull()
  })
})
