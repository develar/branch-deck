import { logError } from "@/lib/errors"

/**
 * A persistent store as seen by the cross-window sync layer (main-window persistence or sub-window proxy).
 * Created by `createPersistentStore`; the sync layer never touches the zustand store directly.
 */
export interface StoreRegistration {
  readonly key: string
  readonly isArray: boolean
  readonly isMainOnly: boolean
  /** `schema.parse({})` for object stores (used to persist only non-default fields), `null` for array stores */
  readonly defaults: Record<string, unknown> | null
  /** Zod-validates a persisted or transmitted value; `null`/`undefined` parse as the empty value (`[]` or `{}`) */
  parse: (value: unknown) => unknown
  /** Plain current value: the array for array stores (not `{ items }`), the object for object stores */
  getValue: () => unknown
  /** Replaces the state with an already parsed value without notifying `subscribeLocal` listeners */
  applyExternal: (parsedValue: unknown) => void
  /** Subscribes to local state changes; changes made by `applyExternal` are filtered out */
  subscribeLocal: (listener: () => void) => () => void
}

/**
 * Applies a value received from another window or from disk, unless it is invalid or equal to the current state.
 */
export function applyIncomingValue(registration: StoreRegistration, value: unknown): void {
  let parsed: unknown
  try {
    parsed = registration.parse(value)
  }
  catch (error) {
    logError(`Invalid value received for ${registration.key}, ignoring`, error)
    return
  }

  if (stableStringify(parsed) !== stableStringify(registration.getValue())) {
    registration.applyExternal(parsed)
  }
}

/**
 * JSON serialization with sorted object keys, so that two structurally equal values
 * produce the same string regardless of property order (Zod output order vs. spread order).
 * `undefined` properties are dropped, as `JSON.stringify` does.
 */
export function stableStringify(value: unknown): string {
  return JSON.stringify(value ?? null, (_key, it: unknown) => {
    if (it !== null && typeof it === "object" && !Array.isArray(it)) {
      const sorted: Record<string, unknown> = {}
      for (const key of Object.keys(it).sort()) {
        sorted[key] = (it as Record<string, unknown>)[key]
      }
      return sorted
    }
    return it
  })
}

// bounds memory if echoes never arrive (e.g. main-only keys are not broadcast)
const MAX_PENDING_PER_KEY = 16

/**
 * Remembers values this window has written so that the change notification that comes back for the same value
 * (LazyStore.onChange in the main window, `store-snapshot-update` in a sub-window) can be recognized and ignored.
 *
 * Ignoring an echo matters when several writes are in flight: applying the echo of an older write
 * would roll the local state back to a stale value until the newer echo arrives.
 */
export class EchoTracker {
  private readonly pending = new Map<string, string[]>()

  record(key: string, serializedValue: string): void {
    let list = this.pending.get(key)
    if (list === undefined) {
      list = []
      this.pending.set(key, list)
    }
    list.push(serializedValue)
    if (list.length > MAX_PENDING_PER_KEY) {
      list.splice(0, list.length - MAX_PENDING_PER_KEY)
    }
  }

  /**
   * Returns `true` if the value is an echo of a recorded write. The matched entry and all older ones are dropped,
   * as notifications arrive in write order and older writes are superseded.
   */
  consume(key: string, serializedValue: string): boolean {
    const list = this.pending.get(key)
    if (list === undefined) {
      return false
    }
    const index = list.indexOf(serializedValue)
    if (index === -1) {
      return false
    }
    list.splice(0, index + 1)
    return true
  }
}
