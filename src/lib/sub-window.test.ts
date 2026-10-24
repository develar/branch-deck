import { beforeAll, describe, expect, it, vi } from "vite-plus/test"
import type { SettingsChange } from "@/lib/bindings"

const { calls, listening } = vi.hoisted(() => {
  let listen = () => {}
  return {
    calls: { loaded: 0, ready: [] as number[], lines: [] as string[], accepted: [] as SettingsChange[], order: [] as string[] },
    // the page listens to the settings once `listen` resolves
    listening: { promise: new Promise<void>(resolve => (listen = resolve)), resolve: () => listen() },
  }
})

vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ label: "commit-review", close: async () => {} }) }))
vi.mock("@tauri-apps/plugin-log", () => ({
  info: async (line: string) => {
    calls.lines.push(line)
  },
}))
vi.mock("@/lib/errors", () => ({ logError: () => {} }))
vi.mock("@/stores/settings/persistent-store", () => ({
  listeningToSettings: listening.promise,
  acceptSettingsChanges: (changes: SettingsChange[]) => {
    calls.accepted.push(...changes)
    calls.order.push("settings")
  },
}))
vi.mock("@/lib/bindings", () => ({
  commands: {
    subWindowLoaded: async () => {
      calls.loaded++
      // Actual Size of the main window while the page loaded
      return { status: "ok", data: { update: null, settings: [{ version: 4, key: "appSettings", value: {}, origin: "main" }] } }
    },
    subWindowReady: async (openId: number) => {
      calls.ready.push(openId)
      calls.order.push("ready")
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
  it("takes the settings changed while the page loaded once it listens, before it says its content settled", async () => {
    // the content settles before the page listens: the load and the show wait for the listener
    markSubWindowContentReady()
    await Promise.resolve()
    expect(calls.loaded).toBe(0)
    expect(calls.ready).toEqual([])
    listening.resolve()
    await vi.waitFor(() => expect(calls.ready).toEqual([7]))
    // the changes made while the page loaded, applied before the window shows
    expect(calls.accepted.map(change => change.version)).toEqual([4])
    expect(calls.order).toEqual(["settings", "ready"])
  })

  it("says the page loaded and its content settled without an animation frame, once per open", async () => {
    expect(calls.loaded).toBe(1)
    markSubWindowContentReady()
    expect(calls.lines).toHaveLength(1)
    expect(calls.lines[0]).toMatch(/^commit-review open \d+ ms: loaded \d+$/)
    // the next open of the window settles on its own
    page.__SUB_WINDOW_OPEN__ = { id: 8, openedAt: Date.now() }
    markSubWindowContentReady()
    await vi.waitFor(() => expect(calls.ready).toEqual([7, 8]))
    expect(calls.loaded).toBe(1)
  })
})
