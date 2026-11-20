// @spec openspec/specs/repository/ide-embedding/spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { resetDock, setDockProgress } from "@/lib/dock"

const { calls } = vi.hoisted(() => ({ calls: { invoked: [] as unknown[], tauri: [] as unknown[] } }))

vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (cmd: string, args: unknown) => {
    calls.invoked.push([cmd, args])
  },
}))
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setProgressBar: async (state: unknown) => {
      calls.tauri.push(state)
    },
  }),
  ProgressBarStatus: { None: "none", Normal: "normal", Indeterminate: "indeterminate" },
}))
vi.mock("@/lib/platform", () => ({ isWindows: false }))
// the tab of the main view in the IDE shows a text before the percent
vi.mock("@/lib/title-bar", () => ({ hasProgressText: true }))

beforeEach(() => {
  resetDock()
  calls.invoked = []
  calls.tauri = []
})

describe("setDockProgress in the IDE", () => {
  it("names what runs before the percent, a progress without an estimate by its text alone, and clears without a text", async () => {
    await setDockProgress({ status: "normal", progress: 37, kind: "safePush" })
    // the same progress again: no call; another kind at the same percent: a call
    await setDockProgress({ status: "normal", progress: 37, kind: "safePush" })
    await setDockProgress({ status: "normal", progress: 37, kind: "dryRun" })
    await setDockProgress({ status: "indeterminate", kind: "dryRun" })
    await setDockProgress({ status: "normal", progress: 40, kind: "update" })
    await setDockProgress({ status: "none" })
    expect(calls.invoked).toEqual([
      ["plugin:window|set_progress_bar", { value: { status: "normal", progress: 37, text: "Safe Push" } }],
      ["plugin:window|set_progress_bar", { value: { status: "normal", progress: 37, text: "Dry Run" } }],
      ["plugin:window|set_progress_bar", { value: { status: "indeterminate", text: "Dry Run" } }],
      ["plugin:window|set_progress_bar", { value: { status: "normal", progress: 40, text: "Update" } }],
      ["plugin:window|set_progress_bar", { value: { status: "none" } }],
    ])
    // the Tauri call of the desktop app, which has no text, is not made
    expect(calls.tauri).toEqual([])
  })
})
