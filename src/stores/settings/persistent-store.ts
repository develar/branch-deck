import type { z } from "zod"
import { createStore } from "zustand/vanilla"
import type { StoreApi } from "zustand/vanilla"
import { logError } from "@/lib/errors"
import { getRawStoreValue, registerStore } from "./app-store"

/** State of an array store; zustand state must be an object, so the array is wrapped. */
export interface ArrayStoreState<T> {
  items: T[]
}

interface PersistentStoreOptions<S> {
  schema: S
  isArray: boolean
  /** Not broadcast to sub-windows */
  isMainOnly: boolean
}

/**
 * Creates a zustand vanilla store that is initialized synchronously from `window.__TAURI_STORE__`,
 * persisted to `settings.json` by the main window and synchronized across windows.
 * The schema is applied to every value read from disk or received from another window.
 */
export function createPersistentStore<S extends z.ZodType<Record<string, unknown>>>(key: string, options: PersistentStoreOptions<S> & { isArray: false }): StoreApi<z.output<S>>
export function createPersistentStore<S extends z.ZodType<unknown[]>>(
  key: string,
  options: PersistentStoreOptions<S> & { isArray: true },
): StoreApi<ArrayStoreState<z.output<S>[number]>>
export function createPersistentStore(key: string, { schema, isArray, isMainOnly }: PersistentStoreOptions<z.ZodType>): StoreApi<unknown> {
  const parse = (value: unknown): unknown => schema.parse(value ?? (isArray ? [] : {}))
  const toState = (value: unknown): object => (isArray ? { items: value } : (value as object))

  let initialValue: unknown
  try {
    initialValue = parse(getRawStoreValue(key))
  } catch (error) {
    logError(`Invalid stored data for ${key}, using defaults`, error)
    initialValue = parse(undefined)
  }

  const store = createStore<object>(() => toState(initialValue))

  // set while applying a value from disk or another window, so the sync subscriber does not send it back
  let applyingExternal = false

  registerStore({
    key,
    isArray,
    isMainOnly,
    defaults: isArray ? null : (parse(undefined) as Record<string, unknown>),
    parse,
    getValue: () => {
      const state = store.getState()
      return isArray ? (state as ArrayStoreState<unknown>).items : state
    },
    applyExternal: parsedValue => {
      applyingExternal = true
      try {
        store.setState(toState(parsedValue), true)
      } finally {
        applyingExternal = false
      }
    },
    subscribeLocal: listener =>
      store.subscribe(() => {
        if (!applyingExternal) {
          listener()
        }
      }),
  })

  return store
}
