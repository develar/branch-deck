// @spec spec/repository/settings.spec.md
import { describe, expect, it, vi } from "vite-plus/test"
import { z } from "zod"
import type { SettingsChange, SettingsPatch } from "@/lib/bindings"
import { acceptSettingsChanges, createPersistentStore } from "@/stores/settings/persistent-store"

interface SetCall {
  patch: SettingsPatch
  answer: (change: SettingsChange | { error: string }) => Promise<void>
}

const { sets, listeners, errors, loadChanges } = vi.hoisted(() => {
  // the snapshot the window started from, at version 3 of the writer
  vi.stubGlobal("window", {
    __TAURI_STORE__: { loaded: { a: "stored" }, review: { zoom: 1.1 } },
    __TAURI_STORE_VERSION__: 3,
  })
  return {
    // the `set_setting` calls in the order they reach the backend, each answered when the test answers it
    sets: [] as SetCall[],
    listeners: new Map<string, (event: { payload: unknown }) => void>(),
    errors: [] as string[],
    // the changes of a key the writer made, as the load of the window hands them over (`sub_window_loaded`), the one
    // before the snapshot too
    loadChanges: [
      { version: 2, key: "loaded", value: { a: "older than the snapshot" }, origin: "main" },
      { version: 5, key: "loaded", value: { a: "while it loaded" }, origin: "main" },
    ] as SettingsChange[],
  }
})

vi.mock("@tauri-apps/api/event", () => ({
  listen: async (name: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(name, handler)
    return () => {}
  },
}))
vi.mock("@/lib/window-label", () => ({ windowLabel: "review", isMainWindow: false }))
vi.mock("@/lib/errors", () => ({
  logError: (title: string) => errors.push(title),
  notifyError: (title: string) => errors.push(title),
}))
vi.mock("@/lib/bindings", () => ({
  commands: {
    setSetting: ({ patch }: { patch: SettingsPatch }) =>
      new Promise(resolve => {
        sets.push({
          patch,
          answer: async change => {
            resolve("error" in change ? { status: "error", error: { message: change.error } } : { status: "ok", data: change })
            // the page takes the answer in a microtask
            await Promise.resolve()
            await Promise.resolve()
          },
        })
      }),
  },
}))

const ObjectSchema = z.object({ a: z.string().default("default"), b: z.boolean().default(false), zoom: z.number().default(1) })

function objectStore(key: string) {
  return createPersistentStore(key, { schema: ObjectSchema, isArray: false, isMainOnly: false })
}

/** A change the writer sends to every window */
function broadcast(change: SettingsChange): void {
  listeners.get("settings_changed")?.({ payload: change })
}

function lastSet(): SetCall {
  const call = sets.at(-1)
  if (call === undefined) {
    throw new Error("nothing was sent")
  }
  return call
}

describe("persistent store", () => {
  it("sends the fields a change made, not the whole object, a field back at its default as null", () => {
    const store = objectStore("fields")
    store.setState({ a: "changed" })
    expect(lastSet().patch).toEqual({ kind: "fields", key: "fields", fields: { a: "changed" } })
    void lastSet().answer({ version: 4, key: "fields", value: { a: "changed" }, origin: "review" })
  })

  it("sends the next change only after the answer of the one before, the changes made meanwhile as one", async () => {
    const store = objectStore("order")
    store.setState({ a: "1" })
    store.setState({ a: "2" })
    store.setState({ b: true })
    expect(sets.filter(it => it.patch.key === "order")).toHaveLength(1)
    await lastSet().answer({ version: 10, key: "order", value: { a: "1" }, origin: "review" })
    expect(lastSet().patch).toEqual({ kind: "fields", key: "order", fields: { a: "2", b: true } })
    // the answer of the first keeps the newer local fields on top
    expect(store.getState()).toMatchObject({ a: "2", b: true })
    await lastSet().answer({ version: 11, key: "order", value: { a: "2", b: true }, origin: "review" })
    store.setState({ a: "default" })
    expect(lastSet().patch).toEqual({ kind: "fields", key: "order", fields: { a: null } })
  })

  it("keeps the field of its own change the writer has not answered when another window's change of the key arrives", async () => {
    const store = objectStore("review")
    expect(store.getState().zoom).toBe(1.1)
    // the review window hides its sidebar while the main window's change of another field is on its way
    store.setState({ b: true })
    broadcast({ version: 20, key: "review", value: { zoom: 1.1, a: "main" }, origin: "main" })
    expect(store.getState()).toEqual({ a: "main", b: true, zoom: 1.1 })
    // the writer applied both: its answer has both fields
    await lastSet().answer({ version: 21, key: "review", value: { zoom: 1.1, a: "main", b: true }, origin: "review" })
    expect(store.getState()).toEqual({ a: "main", b: true, zoom: 1.1 })
  })

  it("never rolls a key back with a late echo of an older change", async () => {
    const store = objectStore("echo")
    store.setState({ zoom: 1.2 })
    await lastSet().answer({ version: 31, key: "echo", value: { zoom: 1.2 }, origin: "review" })
    // the main window's Actual Size, then the late broadcast of the review window's own older change
    broadcast({ version: 32, key: "echo", value: {}, origin: "main" })
    broadcast({ version: 31, key: "echo", value: { zoom: 1.2 }, origin: "review" })
    expect(store.getState().zoom).toBe(1)
    // a later change equal to the window's own earlier value still arrives
    broadcast({ version: 33, key: "echo", value: { zoom: 1.2 }, origin: "main" })
    expect(store.getState().zoom).toBe(1.2)
  })

  it("takes the changes the load of the window hands over by their versions, before its store exists too", () => {
    // the change older than the snapshot changes nothing, the one made while the page loaded is taken
    acceptSettingsChanges(loadChanges)
    const store = objectStore("loaded")
    expect(store.getState().a).toBe("while it loaded")
    // its late broadcast changes nothing either
    acceptSettingsChanges(loadChanges.slice(0, 1))
    broadcast({ version: 5, key: "loaded", value: { a: "late echo" }, origin: "main" })
    expect(store.getState().a).toBe("while it loaded")
  })

  it("puts the writer's value back when it refuses a change, and says so", async () => {
    const store = objectStore("refused")
    store.setState({ zoom: -1 })
    await lastSet().answer({ error: "appSettings.zoom cannot be -1" })
    expect(store.getState().zoom).toBe(1)
    expect(errors).toContain("Cannot change the settings")
  })

  it("sends an array whole, an empty one as no value", async () => {
    const store = createPersistentStore("list", { schema: z.array(z.string()).default([]), isArray: true, isMainOnly: false })
    store.setState({ items: ["a"] })
    expect(lastSet().patch).toEqual({ kind: "value", key: "list", value: ["a"] })
    store.setState({ items: [] })
    // queued behind the first, sent after its answer
    await lastSet().answer({ version: 40, key: "list", value: ["a"], origin: "review" })
    expect(lastSet().patch).toEqual({ kind: "value", key: "list", value: null })
    expect(store.getState().items).toEqual([])
  })

  it("starts a key only the main window keeps empty in a sub-window and takes none of its changes", () => {
    const store = createPersistentStore("activity", { schema: z.array(z.string()).default([]), isArray: true, isMainOnly: true })
    broadcast({ version: 50, key: "activity", value: ["entry"], origin: "main" })
    expect(store.getState().items).toEqual([])
  })
})
