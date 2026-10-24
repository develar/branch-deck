// @spec spec/repository/settings.spec.md
import { describe, expect, it, vi } from "vite-plus/test"
import { z } from "zod"
import type { SettingsChange, SettingsPatch } from "@/lib/bindings"
import { createPersistentStore } from "@/stores/settings/persistent-store"
import type { EventHandler } from "@/test-support/settings-writer-mock"

const { sets, listeners, notified, logged, catchUp } = vi.hoisted(() => {
  // a reload of the main window: its init script has the snapshot of the app's start, version 0
  vi.stubGlobal("window", {
    __TAURI_STORE__: { recentProjects: ["/a"], unchanged: ["/x"], panel: { a: "start" } },
    __TAURI_STORE_VERSION__: 0,
  })
  let answer: (changes: SettingsChange[]) => void = () => {}
  return {
    // the `set_setting` calls in the order they reach the backend
    sets: [] as SettingsPatch[],
    listeners: new Map<string, EventHandler>(),
    notified: [] as string[],
    logged: [] as string[],
    // the answer of `get_settings_changes`, which the test gives
    catchUp: {
      promise: new Promise<{ status: "ok"; data: SettingsChange[] }>(resolve => {
        answer = changes => resolve({ status: "ok", data: changes })
      }),
      answer: (changes: SettingsChange[]) => answer(changes),
    },
  }
})

vi.mock("@tauri-apps/api/event", async () => (await import("@/test-support/settings-writer-mock")).settingsWriterMocks.event(listeners))
// the window of a unit test has no label: it is the main window (`@/lib/window-label`)
vi.mock("@/lib/errors", async () => (await import("@/test-support/settings-writer-mock")).settingsWriterMocks.errors(notified, logged))
vi.mock("@/lib/bindings", () => ({
  commands: {
    getSettingsChanges: (version: number) => {
      expect(version).toBe(0)
      return catchUp.promise
    },
    // never answered: the test looks only at what is sent
    setSetting: ({ patch }: { patch: SettingsPatch }) => {
      sets.push(patch)
      return new Promise(() => {})
    },
  },
}))

/** The page takes an answer of the backend in a few microtasks */
async function settle(): Promise<void> {
  for (let i = 0; i < 10; i++) {
    await Promise.resolve()
  }
}

describe("persistent store of the main window", () => {
  it("tells the user in the main window that the settings could not be saved", async () => {
    await settle()
    const saveFailed = listeners.get("settings_save_failed")
    expect(saveFailed).toBeDefined()
    saveFailed!({ payload: "cannot save the settings to settings.json: Read-only file system" })
    expect(notified).toEqual(["Cannot save the settings"])
  })

  it("holds its changes until it caught up with the writer, and drops a whole array the writer has a newer one of", async () => {
    const recent = createPersistentStore("recentProjects", { schema: z.array(z.string()).default([]), isArray: true, isMainOnly: true })
    const unchanged = createPersistentStore("unchanged", { schema: z.array(z.string()).default([]), isArray: true, isMainOnly: true })
    const panel = createPersistentStore("panel", {
      schema: z.object({ a: z.string().default(""), b: z.boolean().default(false) }),
      isArray: false,
      isMainOnly: false,
    })
    await settle()
    // made from the snapshot of the app's start, before the writer's changes since it are known
    recent.setState({ items: ["/b", "/a"] })
    unchanged.setState({ items: ["/y", "/x"] })
    panel.setState({ b: true })
    expect(sets).toEqual([])

    // since the app started, another project was opened, and another window changed a field of the panel
    catchUp.answer([
      { version: 4, key: "recentProjects", value: ["/c", "/a"], origin: "main" },
      { version: 5, key: "panel", value: { a: "review" }, origin: "review" },
    ])
    await settle()

    // the array built on the stale snapshot would overwrite /c: the writer's value stays
    expect(recent.getState().items).toEqual(["/c", "/a"])
    expect(logged).toContain("Dropped a change of recentProjects made before the window caught up with the settings")
    // a key the writer did not change since the snapshot goes whole, a field on top of the writer's value
    expect(sets).toEqual([
      { kind: "value", key: "unchanged", value: ["/y", "/x"] },
      { kind: "fields", key: "panel", fields: { b: true } },
    ])
    expect(panel.getState()).toEqual({ a: "review", b: true })

    // caught up: a change goes at once
    recent.setState({ items: ["/d", "/c", "/a"] })
    expect(sets.at(-1)).toEqual({ kind: "value", key: "recentProjects", value: ["/d", "/c", "/a"] })
  })
})
