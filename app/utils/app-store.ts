import { LazyStore } from "@tauri-apps/plugin-store"
import { WebviewWindow } from "@tauri-apps/api/webviewWindow"
import { emit, listen } from "@tauri-apps/api/event"
import pDebounce from "p-debounce"
import { SubWindowAppStore } from "./SubWindowAppStore"
import type { z } from "zod"

// Metadata for reactive stores
export interface ReactiveStoreMetadata {
  isArray: boolean
  schema: z.ZodTypeAny
  isMainOnly: boolean
}

// Helper to update both raw snapshot and reactive store if exists
export function updateSnapshot(
  snapshot: Map<string, unknown>,
  reactiveStores: Map<string, ReactiveStoreMetadata>,
  key: string,
  value: unknown,
): void {
  const metadata = reactiveStores.get(key)

  if (metadata) {
    // We have a reactive store - always update it to stay in sync
    const reactiveObj = snapshot.get(key)
    if (reactiveObj && isReactive(reactiveObj)) {
      // Update existing reactive object with new value (including null/undefined)
      updateReactiveState(reactiveObj, metadata.isArray, metadata.schema, value)
    }
    else {
      // Inconsistent state: metadata exists but no reactive object
      // Create reactive object from the new value
      const parsedState = metadata.schema.parse(value)
      const newReactiveObj = metadata.isArray
        ? reactive(parsedState as unknown[])
        : reactive(parsedState as Record<string, unknown>)
      snapshot.set(key, newReactiveObj)
    }
  }
  else {
    // No reactive store - update snapshot with raw data
    if (value === null || value === undefined) {
      snapshot.delete(key)
    }
    else {
      snapshot.set(key, value)
    }
  }
}

// Helper to register reactive store - shared implementation
export function createReactiveStore<T extends z.ZodTypeAny>(
  key: string,
  metadata: ReactiveStoreMetadata & { schema: T },
  snapshot: Map<string, unknown>,
  reactiveStores: Map<string, ReactiveStoreMetadata>,
): z.infer<T> {
  // Check if reactive store already exists
  const existingValue = snapshot.get(key)
  if (existingValue && isReactive(existingValue)) {
    // Already have a reactive store, just update metadata and return it
    reactiveStores.set(key, metadata)
    return existingValue as z.infer<T>
  }

  // Create reactive state with parsed data
  const reactiveState = reactive(metadata.isArray
    ? metadata.schema.parse(existingValue || []) as unknown[]
    : metadata.schema.parse(existingValue || {}) as Record<string, unknown>)

  // Store reactive object in snapshot
  snapshot.set(key, reactiveState)
  // Store metadata separately
  reactiveStores.set(key, metadata)

  return reactiveState as z.infer<T>
}

// Helper to update reactive object/array in place
export function updateReactiveState(reactiveObj: unknown, isArray: boolean, schema: z.ZodTypeAny, value: unknown): void {
  if (isArray) {
    const newValue = schema.parse(value || [])
    // replace array contents
    const arr = reactiveObj as unknown[]
    if (newValue == 0) {
      arr.splice(0)
    }
    else {
      arr.splice(0, arr.length, ...(newValue as unknown[]))
    }
  }
  else {
    // update object properties
    const obj = reactiveObj as Record<string, unknown>
    const newObj = schema.parse(value || {}) as Record<string, unknown>

    // remove properties that are not in the new object
    Object.keys(obj).forEach((k) => {
      if (!(k in newObj)) {
        obj[k] = undefined
      }
    })

    // add/update properties from new object
    Object.assign(obj, newObj)
  }
}

// Interface that both main and sub-window stores implement
export interface IAppStore {
  // Generic store methods
  get<T>(key: string): T | undefined // Now synchronous
  set(key: string, value: unknown): Promise<void>
  registerReactiveStore<T extends z.ZodTypeAny>(key: string, metadata: ReactiveStoreMetadata & { schema: T }): z.infer<T> // Register reactive stores - returns the reactive object
  getStoreSnapshot(): Record<string, unknown> // Get current store state for sub-window initialization
}

// Shared function to create snapshot from preloaded store data
export function createSnapshotFromStore(): Map<string, unknown> {
  return (typeof window !== "undefined" && window.__TAURI_STORE__)
    ? new Map(Object.entries(window.__TAURI_STORE__))
    : new Map()
}

// Main window implementation - direct store access
class MainAppStore implements IAppStore {
  public store: LazyStore // Public so handlers can access it
  public snapshot: Map<string, unknown> // Current state snapshot for sync access
  private reactiveStores: Map<string, ReactiveStoreMetadata> = new Map() // Private map for reactive store metadata

  constructor() {
    this.store = new LazyStore("settings.json")
    this.snapshot = createSnapshotFromStore()

    // Listen for changes and broadcast to all windows
    void this.store.onChange(async (key, value) => {
      // Update snapshot and reactive stores
      updateSnapshot(this.snapshot, this.reactiveStores, key, value)

      // Broadcast to sub-windows (skip main-only stores)
      const metadata = this.reactiveStores.get(key)
      if (!metadata?.isMainOnly) {
        await emit("store-snapshot-update", { key, value })
      }
    })
  }

  // Synchronous get - returns immediately from snapshot
  get<T>(key: string): T | undefined {
    return this.snapshot.get(key) as T
  }

  // Register a reactive store - uses shared implementation
  registerReactiveStore<T extends z.ZodTypeAny>(key: string, metadata: ReactiveStoreMetadata & { schema: T }): z.infer<T> {
    const reactiveState = createReactiveStore(key, metadata, this.snapshot, this.reactiveStores)

    // Parse empty value to get defaults for comparison - only needed for objects
    const defaultsObj = metadata.isArray ? null : metadata.schema.parse({}) as Record<string, unknown>

    // Create debounced persistence function with proper async handling
    const debouncedPersist = pDebounce(async () => {
      try {
        const currentState = reactiveState

        // Handle null/undefined - delete from storage
        if (currentState == null) {
          await this.store.delete(key)
          return
        }

        const raw = toRaw(currentState)

        // Debug logging in test mode
        if (process.env.NUXT_PUBLIC_TEST_MODE) {
          console.log(`[PersistentStore] ${key} changed:`, raw)
        }

        // Validate and normalize the data through Zod
        // This ensures data integrity and handles any invalid mutations
        let validated: unknown
        try {
          validated = metadata.schema.parse(raw)
        }
        catch (error) {
          logError(`Invalid data for ${key}, skipping save`, error)
          return
        }

        let valueToSave: unknown

        // For arrays, check if empty
        if (metadata.isArray) {
          // validated is guaranteed to be an array after schema.parse
          valueToSave = (validated as unknown[]).length === 0 ? null : validated
        }
        else if (defaultsObj) {
          // For objects, check if all values are defaults
          const validatedObj = validated as Record<string, unknown>
          const isAllDefaults = Object.entries(validatedObj).every(
            ([k, v]) => v === defaultsObj[k],
          )

          if (isAllDefaults) {
            valueToSave = null
          }
          else {
            // Only save non-default values
            const toSave: Record<string, unknown> = {}
            Object.entries(validatedObj).forEach(([k, v]) => {
              if (v !== defaultsObj[k]) {
                toSave[k] = v
              }
            })
            valueToSave = toSave
          }
        }
        else {
          // Edge case: not an array and no defaults object
          valueToSave = validated
        }

        // Directly persist to store without going through this.set to avoid circular updates
        if (valueToSave === null) {
          await this.store.delete(key)
        }
        else {
          await this.store.set(key, valueToSave)
        }
      }
      catch (error) {
        logError(`Failed to save ${key} settings`, error)
      }
    }, 500)

    // Set up watcher for persistence (only in main window)
    watch(
      () => reactiveState,
      () => {
        // Call the debounced persist function
        // p-debounce ensures async operations complete before next one starts
        void debouncedPersist()
      },
      { flush: "post", deep: true },
    )

    return reactiveState
  }

  // Get current store state for sub-window initialization
  getStoreSnapshot(): Record<string, unknown> {
    return Object.fromEntries(this.snapshot)
  }

  // Async set - updates snapshot immediately, persists in background
  async set(key: string, value: unknown): Promise<void> {
    // Check if we have a reactive store for this key
    const metadata = this.reactiveStores.get(key)
    if (metadata) {
      // It's a reactive store - update it in place
      const reactiveObj = this.snapshot.get(key)
      if (reactiveObj && isReactive(reactiveObj)) {
        updateReactiveState(reactiveObj, metadata.isArray, metadata.schema, value)
      }
      // Persist the raw value
      await this.store.set(key, value)
    }
    else {
      // Raw value
      if (value === null || value === undefined) {
        this.snapshot.delete(key)
        await this.store.delete(key)
      }
      else {
        this.snapshot.set(key, value)
        await this.store.set(key, value)
      }
    }
  }
}

// Factory function to create the appropriate store implementation
function createAppStore(): IAppStore {
  try {
    const currentWindow = WebviewWindow.getCurrent()
    const label = currentWindow.label
    if (label === "main") {
      return new MainAppStore()
    }
    else {
      return new SubWindowAppStore()
    }
  }
  catch {
    // If we can't determine the window, assume we're in main
    return new MainAppStore()
  }
}

// Store request handlers for main window
export function initializeStoreHandlers() {
  // Use the existing appStore instance
  const mainStore = appStore

  // Only initialize handlers if appStore is MainAppStore
  if (!(mainStore instanceof MainAppStore)) {
    // Don't set up handlers in sub-windows
    return
  }

  // Handle store set requests from sub-windows
  // NOTE: We use direct store.set/delete here (not mainStore.set) because:
  // 1. Sub-window requests should only persist to disk
  // 2. The store.onChange handler will automatically update reactive stores and broadcast to all windows
  // 3. Using mainStore.set() would cause double updates (once here, once from onChange)
  void listen<{ requestId: string, key: string, value: unknown }>("store-set-request", async (event) => {
    try {
      const { requestId, key, value } = event.payload
      if (value === null || value === undefined) {
        await mainStore.store.delete(key)
      }
      else {
        await mainStore.store.set(key, value)
      }

      await emit("store-response", {
        requestId,
        success: true,
      })
    }
    catch (error) {
      await emit("store-response", {
        requestId: event.payload.requestId,
        success: false,
        error: error instanceof Error ? error.message : "Unknown error",
      })
    }
  })
}

// Create a singleton instance using factory
export const appStore = createAppStore()
