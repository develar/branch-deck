import { describe, expect, it, vi } from "vite-plus/test"
import { initializeStoreHandlers, registerStore } from "@/stores/settings/app-store"
import type { StoreRegistration } from "@/stores/settings/sync-utils"

interface StoreWrite {
  key: string
  value: unknown
  finish: (error?: Error) => void
}

const { listeners, writes, responses } = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  // the `set` calls in the order they reach the store, each answered when the test finishes it
  writes: [] as StoreWrite[],
  responses: [] as unknown[],
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

const DEFAULTS = { page: "general" }
let state: Record<string, unknown> = { ...DEFAULTS }
registerStore({
  key: "pages",
  isArray: false,
  isMainOnly: false,
  defaults: DEFAULTS,
  parse: value => ({ ...DEFAULTS, ...(value as Record<string, unknown> | null) }),
  getValue: () => state,
  applyExternal: parsed => {
    state = parsed as Record<string, unknown>
  },
  subscribeLocal: () => () => {},
} satisfies StoreRegistration)
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
})
