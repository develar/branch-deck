import { describe, expect, it, vi } from "vite-plus/test"
import { initializeStoreHandlers, registerStore } from "@/stores/settings/app-store"
import { SubWindowStoreSync } from "@/stores/settings/sub-window-app-store"
import type { StoreRegistration } from "@/stores/settings/sync-utils"

interface StoreWrite {
  key: string
  value: unknown
  finish: (error?: Error) => void
}

const { listeners, writes, responses, emitted } = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  // the `set` calls in the order they reach the store, each answered when the test finishes it
  writes: [] as StoreWrite[],
  responses: [] as unknown[],
  emitted: [] as { name: string; payload: unknown }[],
}))

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@/components/ui/toast", () => ({ toast: { add: () => "toast", close: () => {} } }))
vi.mock("@tauri-apps/api/webviewWindow", () => ({ WebviewWindow: { getCurrent: () => ({ label: "main" }) } }))
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (name: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(name, handler)
    return () => {}
  },
  emit: async (name: string, payload: unknown) => {
    emitted.push({ name, payload })
    if (name === "store-response") {
      responses.push(payload)
    }
  },
}))
vi.mock("@tauri-apps/plugin-store", () => ({
  LazyStore: class {
    onChange = async () => () => {}
    set = (key: string, value: unknown) =>
      new Promise<void>((resolve, reject) => {
        writes.push({ key, value, finish: error => (error === undefined ? resolve() : reject(error)) })
      })
    delete = async () => {}
  },
}))

/** An object store `key` of the defaults `defaults`, its state read and written through `holder` */
function objectStore(key: string, defaults: Record<string, unknown>, holder: { state: Record<string, unknown>; local: () => void }): StoreRegistration {
  return {
    key,
    isArray: false,
    isMainOnly: false,
    defaults,
    parse: value => ({ ...defaults, ...(value as Record<string, unknown> | null) }),
    getValue: () => holder.state,
    applyExternal: parsed => {
      holder.state = parsed as Record<string, unknown>
    },
    subscribeLocal: listener => {
      holder.local = listener
      return () => {}
    },
  }
}

const DEFAULTS = { page: "general" }
const pages = { state: { ...DEFAULTS } as Record<string, unknown>, local: () => {} }
let state: Record<string, unknown> = pages.state
registerStore({
  ...objectStore("pages", DEFAULTS, pages),
  getValue: () => state,
  applyExternal: parsed => {
    state = parsed as Record<string, unknown>
  },
})
initializeStoreHandlers()

/** A write of the Settings window, as the main window receives it */
function requestFromSubWindow(requestId: string, page: string): void {
  const handler = listeners.get("store-set-request")
  if (handler === undefined) {
    throw new Error("the main window serves no store-set-request")
  }
  handler({ payload: { requestId, key: "pages", value: { page } } })
}

/** The `index`-th `set` that reached the store */
function storeWrite(index: number): StoreWrite {
  const write = writes[index]
  if (write === undefined) {
    throw new Error(`no write ${index} reached the store`)
  }
  return write
}

describe("MainStoreSync", () => {
  it("writes the settings one at a time, in the order they changed, so the last change is the one on disk", async () => {
    // ↓ ↓ ↑ in the sidebar of Settings: three writes before the first is saved
    requestFromSubWindow("1", "appearance")
    requestFromSubWindow("2", "notifications")
    requestFromSubWindow("3", "appearance")
    expect(state).toEqual({ page: "appearance" })
    await vi.waitFor(() => expect(writes).toHaveLength(1))
    expect(storeWrite(0).value).toEqual({ page: "appearance" })

    storeWrite(0).finish()
    await vi.waitFor(() => expect(writes).toHaveLength(2))
    expect(storeWrite(1).value).toEqual({ page: "notifications" })
    // a failed write is answered as failed, and the next one still reaches the store
    storeWrite(1).finish(new Error("disk full"))
    await vi.waitFor(() => expect(writes).toHaveLength(3))
    expect(storeWrite(2).value).toEqual({ page: "appearance" })
    storeWrite(2).finish()

    await vi.waitFor(() => expect(responses).toHaveLength(3))
    expect(responses).toEqual([
      { requestId: "1", success: true },
      { requestId: "2", success: false, error: "disk full" },
      { requestId: "3", success: true },
    ])
    expect(writes.map(it => it.key)).toEqual(["pages", "pages", "pages"])
  })

  it("takes only the fields a sub-window changed, so a field the main window changed meanwhile stays", async () => {
    // the main window changed `panel` a moment ago; the sub-window's value still has the old one
    state = { page: "general", panel: true }
    listeners.get("store-set-request")!({ payload: { requestId: "4", key: "pages", value: { page: "appearance", panel: false }, fields: ["page"] } })
    expect(state).toEqual({ page: "appearance", panel: true })
    await vi.waitFor(() => expect(writes).toHaveLength(4))
    expect(storeWrite(3).value).toEqual({ page: "appearance", panel: true })
    storeWrite(3).finish()
    await vi.waitFor(() => expect(responses).toHaveLength(4))
  })

  it("answers a sub-window that asks for the settings with those it has now", () => {
    state = { page: "notifications" }
    listeners.get("store-snapshot-request")!({ payload: { label: "review-1" } })
    expect(emitted.at(-1)).toEqual({ name: "store-snapshot", payload: { label: "review-1", snapshot: { pages: { page: "notifications" } } } })
  })
})

const APP_DEFAULTS = { activityPanel: false, reviewSidebar: true }

/** A sub-window whose `appSettings` the test changes as the page would (`change`) */
function subWindow(): { state: () => Record<string, unknown>; change: (update: Record<string, unknown>) => void } {
  const holder = { state: { ...APP_DEFAULTS } as Record<string, unknown>, local: () => {} }
  const registration = objectStore("appSettings", APP_DEFAULTS, holder)
  new SubWindowStoreSync("review", new Map([["appSettings", registration]]), new Map()).attach(registration)
  return {
    state: () => holder.state,
    change: update => {
      holder.state = { ...holder.state, ...update }
      holder.local()
    },
  }
}

function deliver(name: string, payload: unknown): void {
  listeners.get(name)!({ payload })
}

describe("SubWindowStoreSync", () => {
  it("sends the main window the fields a change made, not the whole object", () => {
    const window = subWindow()
    window.change({ reviewSidebar: false })
    expect(emitted.findLast(it => it.name === "store-set-request")?.payload).toMatchObject({
      key: "appSettings",
      value: { activityPanel: false, reviewSidebar: false },
      fields: ["reviewSidebar"],
    })
  })

  it("asks for the settings once it listens, and takes a change the main window made while the page loaded", async () => {
    const window = subWindow()
    await vi.waitFor(() => expect(emitted.findLast(it => it.name === "store-snapshot-request")?.payload).toEqual({ label: "review" }))
    // another window's answer is not this window's
    deliver("store-snapshot", { label: "settings", snapshot: { appSettings: { activityPanel: true, reviewSidebar: false } } })
    expect(window.state()).toEqual({ activityPanel: false, reviewSidebar: true })
    deliver("store-snapshot", { label: "review", snapshot: { appSettings: { activityPanel: true, reviewSidebar: true } } })
    expect(window.state()).toEqual({ activityPanel: true, reviewSidebar: true })
  })

  it("keeps the field of a write the main window has not answered when another window's change arrives", () => {
    const window = subWindow()
    window.change({ reviewSidebar: false })
    const { requestId } = emitted.findLast(it => it.name === "store-set-request")!.payload as { requestId: string }
    // the main window broadcasts its own change, made before it applied this window's write
    deliver("store-snapshot-update", { key: "appSettings", value: { activityPanel: true } })
    expect(window.state()).toEqual({ activityPanel: true, reviewSidebar: false })

    // answered: the main window's value is the truth again
    deliver("store-response", { requestId, success: true })
    deliver("store-snapshot-update", { key: "appSettings", value: { activityPanel: true } })
    expect(window.state()).toEqual({ activityPanel: true, reviewSidebar: true })
  })
})
