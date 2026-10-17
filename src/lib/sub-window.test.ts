import { beforeAll, describe, expect, it, vi } from "vite-plus/test"

const { calls } = vi.hoisted(() => ({ calls: { loaded: 0, ready: [] as number[], lines: [] as string[] } }))

vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ label: "commit-review", close: async () => {} }) }))
vi.mock("@tauri-apps/plugin-log", () => ({
  info: async (line: string) => {
    calls.lines.push(line)
  },
}))
vi.mock("@/lib/errors", () => ({ logError: () => {} }))
vi.mock("@/lib/bindings", () => ({
  commands: {
    subWindowLoaded: async () => {
      calls.loaded++
      return { status: "ok", data: null }
    },
    subWindowReady: async (openId: number) => {
      calls.ready.push(openId)
      return { status: "ok", data: null }
    },
  },
}))

const page = { __SUB_WINDOW_OPEN__: { id: 7, openedAt: Date.now() }, addEventListener: () => {}, dispatchEvent: () => true }

let markSubWindowContentReady: () => void

beforeAll(async () => {
  // a hidden WKWebView runs no animation frames: nothing on the path to the show may wait for one
  vi.stubGlobal("requestAnimationFrame", () => 0)
  vi.stubGlobal("window", page)
  ;({ markSubWindowContentReady } = await import("@/lib/sub-window"))
})

describe("the show of a sub-window", () => {
  it("says the page loaded and its content settled without an animation frame, once per open", () => {
    expect(calls.loaded).toBe(1)
    markSubWindowContentReady()
    markSubWindowContentReady()
    expect(calls.ready).toEqual([7])
    expect(calls.lines).toHaveLength(1)
    expect(calls.lines[0]).toMatch(/^commit-review open \d+ ms: loaded \d+$/)
    // the next open of the window settles on its own
    page.__SUB_WINDOW_OPEN__ = { id: 8, openedAt: Date.now() }
    markSubWindowContentReady()
    expect(calls.ready).toEqual([7, 8])
  })
})
