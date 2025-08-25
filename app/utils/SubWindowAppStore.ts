import { emit, listen } from "@tauri-apps/api/event"
import type { IAppStore, ReactiveStoreMetadata } from "./app-store"
import { createSnapshotFromStore, updateSnapshot, createReactiveStore, updateReactiveState } from "./app-store"
import type { z } from "zod"

// Sub-window implementation - proxy to main window
export class SubWindowAppStore implements IAppStore {
  private requestCounter = 0
  public snapshot: Map<string, unknown>
  private reactiveStores: Map<string, ReactiveStoreMetadata> = new Map()

  constructor() {
    this.snapshot = createSnapshotFromStore()

    // Listen for snapshot updates from main window
    void listen<{ key: string, value: unknown }>("store-snapshot-update", (event) => {
      const { key, value } = event.payload

      // Update snapshot and reactive stores
      updateSnapshot(this.snapshot, this.reactiveStores, key, value)
    })
  }

  // Synchronous get from snapshot
  get<T>(key: string): T | undefined {
    return this.snapshot.get(key) as T
  }

  // Register a reactive store - uses shared implementation
  registerReactiveStore<T extends z.ZodTypeAny>(key: string, metadata: ReactiveStoreMetadata & { schema: T }): z.infer<T> {
    return createReactiveStore(key, metadata, this.snapshot, this.reactiveStores)
  }

  // Get current store state for sub-window initialization
  getStoreSnapshot(): Record<string, unknown> {
    return Object.fromEntries(this.snapshot)
  }

  // Async set - updates snapshot immediately, then syncs with main window
  async set(key: string, value: unknown): Promise<void> {
    // Check if we have a reactive store for this key
    const metadata = this.reactiveStores.get(key)
    if (metadata) {
      // It's a reactive store - update it in place
      const reactiveObj = this.snapshot.get(key)
      if (reactiveObj && isReactive(reactiveObj)) {
        updateReactiveState(reactiveObj, metadata.isArray, metadata.schema, value)
      }
    }
    else {
      // Raw value
      if (value === null || value === undefined) {
        this.snapshot.delete(key)
      }
      else {
        this.snapshot.set(key, value)
      }
    }

    // Sync with main window
    await this.setInMain(key, value)
  }

  // Proxy method to set data via main window
  private async setInMain(key: string, value: unknown): Promise<void> {
    const requestId = `store-set-${++this.requestCounter}`

    // Set up listener for response
    const responsePromise = new Promise<void>((resolve, reject) => {
      const unlisten = listen<{ requestId: string, success: boolean, error?: string }>(
        "store-response",
        (event) => {
          if (event.payload.requestId === requestId) {
            unlisten.then(fn => fn())
            if (event.payload.success) {
              resolve()
            }
            else {
              reject(new Error(event.payload.error || "Store set failed"))
            }
          }
        },
      )
    })

    // Send request to main window
    await emit("store-set-request", { requestId, key, value })

    return responsePromise
  }
}
