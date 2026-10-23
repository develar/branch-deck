import { emit, listen } from "@tauri-apps/api/event"
import { WebviewWindow } from "@tauri-apps/api/webviewWindow"
import { LazyStore } from "@tauri-apps/plugin-store"
import pDebounce from "p-debounce"
import { logError } from "@/lib/errors"
import { SubWindowStoreSync } from "./sub-window-app-store"
import type { StoreSnapshot } from "./sub-window-app-store"
import { applyIncomingValue, EchoTracker, stableStringify } from "./sync-utils"
import type { StoreRegistration } from "./sync-utils"

/*
 * Distributed settings (see docs/design/distributed-settings.md).
 *
 * The main window owns `settings.json` (LazyStore): it persists local changes (debounced, non-default fields only),
 * applies writes requested by sub-windows, and broadcasts every change as `store-snapshot-update`.
 * Sub-windows start from the snapshot injected as `window.__TAURI_STORE__` and proxy writes to the main window.
 */

interface StoreSetRequest {
  requestId: string
  key: string
  value: unknown
  /** The fields of an object store the sub-window changed: only they are taken, the others stay as the main window has them */
  fields?: readonly string[]
}

function detectWindowLabel(): string {
  try {
    return WebviewWindow.getCurrent().label
  } catch {
    // if we can't determine the window, assume we're in main
    return "main"
  }
}

const windowLabel = detectWindowLabel()
export const isMainWindow = windowLabel === "main"

const registry = new Map<string, StoreRegistration>()

// raw values of all keys as injected by the backend (or updated since); registered stores override their keys
const rawSnapshot = new Map<string, unknown>(typeof window !== "undefined" && window.__TAURI_STORE__ ? Object.entries(window.__TAURI_STORE__) : [])

/**
 * The value to write to disk: `null` (delete the key) for empty arrays and all-default objects,
 * otherwise the array or only the fields that differ from the defaults.
 */
function toPersistedValue(registration: StoreRegistration, validated: unknown): unknown {
  if (registration.isArray) {
    return (validated as unknown[]).length === 0 ? null : validated
  }

  const defaults = registration.defaults
  if (defaults === null) {
    return validated
  }

  const toSave: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(validated as Record<string, unknown>)) {
    if (value !== defaults[key]) {
      toSave[key] = value
    }
  }
  return Object.keys(toSave).length === 0 ? null : toSave
}

class MainStoreSync {
  readonly store = new LazyStore("settings.json")
  private readonly echoTracker = new EchoTracker()
  private handlersInitialized = false
  // the end of the last write asked for; the next one starts after it (`write`)
  private lastWrite: Promise<void> = Promise.resolve()

  constructor() {
    this.store
      .onChange((key, value) => {
        this.handleDiskChange(key, value)
      })
      .catch((error: unknown) => {
        logError("Failed to listen to settings changes", error)
      })
  }

  attach(registration: StoreRegistration): void {
    const debouncedPersist = pDebounce(() => this.persist(registration), 500)
    registration.subscribeLocal(() => {
      void debouncedPersist()
    })
  }

  /**
   * Writes to disk in the order the writes were asked for, one at a time: two `set` calls in flight reach the store in
   * either order (Tauri runs async commands concurrently), and the older value would stay on disk. A failed write is
   * its caller's error; the next one still runs.
   */
  private write(key: string, value: unknown): Promise<void> {
    const next = this.lastWrite.then(() => this.writeNow(key, value))
    this.lastWrite = next.catch(() => undefined)
    return next
  }

  /** Writes to disk and remembers the value, so that the resulting `onChange` is not applied back to the store. */
  private async writeNow(key: string, value: unknown): Promise<void> {
    this.echoTracker.record(key, stableStringify(value))
    if (value === null || value === undefined) {
      await this.store.delete(key)
    } else {
      await this.store.set(key, value)
    }
  }

  private async persist(registration: StoreRegistration): Promise<void> {
    const key = registration.key
    try {
      // validate and normalize the data through Zod
      let validated: unknown
      try {
        validated = registration.parse(registration.getValue())
      } catch (error) {
        logError(`Invalid data for ${key}, skipping save`, error)
        return
      }

      await this.write(key, toPersistedValue(registration, validated))
    } catch (error) {
      logError(`Failed to save ${key} settings`, error)
    }
  }

  private handleDiskChange(key: string, value: unknown): void {
    const normalizedValue = value ?? null
    const registration = registry.get(key)
    if (registration === undefined) {
      if (normalizedValue === null) {
        rawSnapshot.delete(key)
      } else {
        rawSnapshot.set(key, normalizedValue)
      }
    } else if (!this.echoTracker.consume(key, stableStringify(normalizedValue))) {
      applyIncomingValue(registration, normalizedValue)
    }

    // broadcast to sub-windows (skip main-only stores), including echoes of our own writes
    if (registration?.isMainOnly !== true) {
      emit("store-snapshot-update", { key, value: normalizedValue }).catch((error: unknown) => {
        logError(`Failed to broadcast ${key} settings`, error)
      })
    }
  }

  /**
   * Handles writes requested by sub-windows. Registered keys are applied to the local store immediately
   * (without re-triggering the debounced persist) and saved in the same non-default form as local changes;
   * the resulting `onChange` broadcasts the value to all sub-windows.
   */
  initializeHandlers(): void {
    if (this.handlersInitialized) {
      return
    }
    this.handlersInitialized = true

    listen<StoreSetRequest>("store-set-request", event => {
      this.applySetRequest(event.payload).catch((error: unknown) => {
        logError(`Failed to answer the ${event.payload.key} settings request of a sub-window`, error)
      })
    }).catch((error: unknown) => {
      logError("Failed to listen to store-set-request", error)
    })

    // a sub-window that listens now asks for what changed while its page loaded
    listen<{ label: string }>("store-snapshot-request", event => {
      const answer: StoreSnapshot = { label: event.payload.label, snapshot: getStoreSnapshot() }
      emit("store-snapshot", answer).catch((error: unknown) => {
        logError(`Failed to send the settings to ${event.payload.label}`, error)
      })
    }).catch((error: unknown) => {
      logError("Failed to listen to store-snapshot-request", error)
    })
  }

  private async applySetRequest({ requestId, key, value, fields }: StoreSetRequest): Promise<void> {
    try {
      const registration = registry.get(key)
      if (registration === undefined) {
        await this.write(key, value)
      } else {
        // the fields the sub-window changed, on the main window's value: a field changed here meanwhile stays
        const merged =
          fields === undefined || registration.isArray
            ? value
            : { ...(registration.getValue() as Record<string, unknown>), ...Object.fromEntries(fields.map(field => [field, (value as Record<string, unknown>)[field]])) }
        const validated = registration.parse(merged)
        applyIncomingValue(registration, validated)
        await this.write(key, toPersistedValue(registration, validated))
      }
      await emit("store-response", { requestId, success: true })
    } catch (error) {
      logError(`Failed to apply ${key} settings from sub-window`, error)
      await emit("store-response", {
        requestId,
        success: false,
        error: error instanceof Error ? error.message : "Unknown error",
      })
    }
  }
}

const sync: MainStoreSync | SubWindowStoreSync = isMainWindow ? new MainStoreSync() : new SubWindowStoreSync(windowLabel, registry, rawSnapshot)

/** Raw value of `key` from the injected snapshot (or a later update), used as the initial state of a store. */
export function getRawStoreValue(key: string): unknown {
  return rawSnapshot.get(key)
}

/**
 * Registers a persistent store with the sync layer. A key registered again (e.g. after HMR) replaces the previous one.
 */
export function registerStore(registration: StoreRegistration): void {
  registry.set(registration.key, registration)
  sync.attach(registration)
}

/**
 * Current settings for sub-window initialization (`storeCache` of `open_sub_window`):
 * parsed state of registered stores (arrays as plain arrays) plus raw values of unregistered keys.
 * Main-only stores are left out: a sub-window never receives their changes.
 */
export function getStoreSnapshot(): Record<string, unknown> {
  const snapshot: Record<string, unknown> = {}
  for (const [key, value] of rawSnapshot) {
    if (registry.get(key)?.isMainOnly !== true) {
      snapshot[key] = value
    }
  }
  for (const [key, registration] of registry) {
    if (!registration.isMainOnly) {
      snapshot[key] = registration.getValue()
    }
  }
  return snapshot
}

/**
 * Starts serving `store-set-request` from sub-windows. No-op in sub-windows; idempotent.
 * Call once at module level of the entry point, not from an effect.
 */
export function initializeStoreHandlers(): void {
  if (sync instanceof MainStoreSync) {
    sync.initializeHandlers()
  }
}
