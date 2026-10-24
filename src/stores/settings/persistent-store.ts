import { listen } from "@tauri-apps/api/event"
import type { z } from "zod"
import { createStore } from "zustand/vanilla"
import type { StoreApi } from "zustand/vanilla"
import { commands } from "@/lib/bindings"
import type { SettingsChange, SettingsPatch } from "@/lib/bindings"
import { logError, notifyError } from "@/lib/errors"
import { stableStringify } from "@/lib/stable-stringify"
import { isMainWindow } from "@/lib/window-label"

/*
 * The settings of a window (see docs/design/distributed-settings.md). The backend has the one writer of `settings.json`
 * (`SettingsService`): every window starts from its snapshot (`window.__TAURI_STORE__` at `__TAURI_STORE_VERSION__`),
 * sends each local change as a patch (`set_setting`: the fields of an object that changed, or the whole array) and
 * applies every change the writer sends to all windows (`settings_changed`) that is newer than the one it applied, the
 * changes made while the page loaded too (`listeningToSettings`, `acceptSettingsChanges`).
 * A change is never rolled back by an older answer, and the fields of a window's own patches that the writer has not
 * answered yet stay on top of a value of another window.
 */

/** State of an array store; zustand state must be an object, so the array is wrapped. */
export interface ArrayStoreState<T> {
  items: T[]
}

interface PersistentStoreOptions<S> {
  schema: S
  isArray: boolean
  /** Only the main window has it: the backend never sends it to a sub-window */
  isMainOnly: boolean
}

type Fields = Record<string, unknown>

/** A key as the sync sees it; the zustand store stays behind `getValue` and `applyExternal` */
interface KeySync {
  readonly key: string
  readonly isMainOnly: boolean
  readonly parse: (value: unknown) => unknown
  /** The plain value: the array of an array store, the object of an object store */
  readonly getValue: () => unknown
  /** Replaces the state with a parsed value without sending it */
  readonly applyExternal: (parsed: unknown) => void
  /** The patch of a local change: what differs from `known` */
  readonly patchOf: (value: unknown) => SettingsPatch | null
  /** The value the store had after the last change it sent or took */
  known: unknown
  /** The patch the writer has not answered yet; the next one waits for its answer, so the writer gets them in order */
  sent: SettingsPatch | null
  /** The local changes made while `sent` is unanswered, merged into one patch */
  queued: SettingsPatch | null
}

/** The value of a key as the writer has it, with the version of the change that gave it (the snapshot's first) */
interface Confirmed {
  value: unknown
  version: number
}

const hasBackend = typeof window !== "undefined"
const snapshotVersion = (hasBackend ? window.__TAURI_STORE_VERSION__ : undefined) ?? 0
const snapshot: Readonly<Record<string, unknown>> = (hasBackend ? window.__TAURI_STORE__ : undefined) ?? {}
const confirmed = new Map<string, Confirmed>(Object.entries(snapshot).map(([key, value]) => [key, { value, version: snapshotVersion }]))

/** The value of `key` in the snapshot the window started from, as the backend built the window with it */
export function snapshotValueOf(key: string): unknown {
  return snapshot[key]
}
const syncs = new Map<string, KeySync>()

/** The fields of `after` that differ from `before`, each with its value, `null` for one that is its default again */
export function changedFields(before: Fields, after: Fields, defaults: Fields): Fields {
  const fields: Fields = {}
  for (const field of new Set([...Object.keys(before), ...Object.keys(after)])) {
    const value = after[field]
    if (stableStringify(before[field]) !== stableStringify(value)) {
      fields[field] = value === undefined || stableStringify(value) === stableStringify(defaults[field]) ? null : value
    }
  }
  return fields
}

/** `patch` applied to `value`, as the writer applies it */
function applyPatch(value: unknown, patch: SettingsPatch): unknown {
  if (patch.kind === "value") {
    return patch.value
  }
  const object = new Map(value !== null && typeof value === "object" && !Array.isArray(value) ? Object.entries(value) : [])
  for (const [field, fieldValue] of Object.entries(patch.fields)) {
    if (fieldValue === null) {
      object.delete(field)
    } else {
      object.set(field, fieldValue)
    }
  }
  return Object.fromEntries(object)
}

/** `patch`, then `next`, as one patch for the writer */
function mergePatch(patch: SettingsPatch | null, next: SettingsPatch): SettingsPatch {
  if (patch === null || next.kind === "value") {
    return next
  }
  if (patch.kind === "value") {
    return { kind: "value", key: next.key, value: applyPatch(patch.value, next) }
  }
  return { kind: "fields", key: next.key, fields: { ...patch.fields, ...next.fields } }
}

/** The state of `sync`: the writer's value with the unanswered local patches on top, unless it is that already */
function refresh(sync: KeySync): void {
  let value: unknown = confirmed.get(sync.key)?.value ?? null
  for (const patch of [sync.sent, sync.queued]) {
    if (patch !== null) {
      value = applyPatch(value, patch)
    }
  }
  let parsed: unknown
  try {
    parsed = sync.parse(value)
  } catch (error) {
    logError(`Invalid value received for ${sync.key}, ignoring`, error)
    return
  }
  sync.known = parsed
  if (stableStringify(parsed) !== stableStringify(sync.getValue())) {
    sync.applyExternal(parsed)
  }
}

/** A change of the writer, from `settings_changed` or the answer of a patch: taken when it is newer than the last one */
function accept(change: SettingsChange): void {
  const current = confirmed.get(change.key)
  if (current !== undefined && change.version <= current.version) {
    return
  }
  confirmed.set(change.key, { value: change.value, version: change.version })
  const sync = syncs.get(change.key)
  if (sync !== undefined && (isMainWindow || !sync.isMainOnly)) {
    refresh(sync)
  }
}

/** Sends `patch` of `sync` to the writer; the answer takes it, then the changes queued meanwhile go as the next one */
function send(sync: KeySync, patch: SettingsPatch): void {
  sync.sent = patch
  const answered = (change: SettingsChange | null, error?: unknown): void => {
    sync.sent = null
    if (change === null) {
      // refused or lost, so the value the store shows goes back to the writer's
      notifyError("Cannot change the settings", error)
    } else {
      accept(change)
    }
    const queued = sync.queued
    sync.queued = null
    if (queued !== null) {
      send(sync, queued)
    }
    refresh(sync)
  }
  void commands.setSetting({ patch }).then(
    result => (result.status === "ok" ? answered(result.data) : answered(null, result.error)),
    (error: unknown) => answered(null, error),
  )
}

/** A change made in this window: its patch to the writer, now or after the answer of the one in flight */
function changedLocally(sync: KeySync): void {
  const value = sync.getValue()
  const patch = sync.patchOf(value)
  sync.known = value
  if (patch === null || !hasBackend) {
    return
  }
  if (sync.sent === null) {
    send(sync, patch)
  } else {
    sync.queued = mergePatch(sync.queued, patch)
  }
}

/**
 * Listens to the writer once per page, at its start: every change it sends; the main window also hears a failed save,
 * which it tells the user. The main window then takes the changes made after the snapshot it started from, which reached
 * no listener (a reload starts from the snapshot of the app's start); a sub-window takes them with its load
 * (`sub_window_loaded` in `@/lib/sub-window`), before its content settles.
 */
function listenToWriter(): Promise<void> {
  if (!hasBackend) {
    return Promise.resolve()
  }
  if (isMainWindow) {
    listen<string>("settings_save_failed", event => {
      notifyError("Cannot save the settings", event.payload)
    }).catch((error: unknown) => {
      logError("Failed to listen to the saves of the settings", error)
    })
  }
  const listening = listen<SettingsChange>("settings_changed", event => {
    accept(event.payload)
  }).then(
    () => {},
    (error: unknown) => {
      logError("Failed to listen to the settings", error)
    },
  )
  if (isMainWindow) {
    void listening
      .then(() => commands.getSettingsChanges(snapshotVersion))
      .then(result => {
        if (result.status === "error") {
          logError("Cannot read the settings changed while the window loaded", result.error)
          return
        }
        acceptSettingsChanges(result.data)
      })
  }
  return listening
}

/** Resolves once the page listens to the changes of the writer (`settings_changed`): any change after it reaches the page */
export const listeningToSettings: Promise<void> = listenToWriter()

/**
 * Takes the changes the writer made after the snapshot the window started from, each by its version: one the page has
 * heard already, or older than the one it applied, changes nothing
 */
export function acceptSettingsChanges(changes: readonly SettingsChange[]): void {
  for (const change of changes) {
    accept(change)
  }
}

/**
 * Creates a zustand vanilla store that is initialized synchronously from the snapshot of the window
 * (`window.__TAURI_STORE__`), sends its changes to the writer of the settings in the backend and takes the changes of
 * every window. The schema is applied to every value read from the snapshot or received from the writer.
 */
export function createPersistentStore<S extends z.ZodType<Record<string, unknown>>>(key: string, options: PersistentStoreOptions<S> & { isArray: false }): StoreApi<z.output<S>>
export function createPersistentStore<S extends z.ZodType<unknown[]>>(
  key: string,
  options: PersistentStoreOptions<S> & { isArray: true },
): StoreApi<ArrayStoreState<z.output<S>[number]>>
export function createPersistentStore(key: string, { schema, isArray, isMainOnly }: PersistentStoreOptions<z.ZodType>): StoreApi<unknown> {
  const parse = (value: unknown): unknown => schema.parse(value ?? (isArray ? [] : {}))
  const toState = (value: unknown): object => (isArray ? { items: value } : (value as object))
  const defaults = isArray ? null : (parse(undefined) as Fields)

  let initialValue: unknown
  try {
    initialValue = parse(isMainOnly && !isMainWindow ? undefined : confirmed.get(key)?.value)
  } catch (error) {
    logError(`Invalid stored data for ${key}, using defaults`, error)
    initialValue = parse(undefined)
  }

  const store = createStore<object>(() => toState(initialValue))
  // set while applying a value of the writer, so the subscriber does not send it back
  let applyingExternal = false

  const sync: KeySync = {
    key,
    isMainOnly,
    parse,
    getValue: () => {
      const state = store.getState()
      return isArray ? (state as ArrayStoreState<unknown>).items : state
    },
    applyExternal: parsed => {
      applyingExternal = true
      try {
        store.setState(toState(parsed), true)
      } finally {
        applyingExternal = false
      }
    },
    patchOf: value => {
      if (defaults === null) {
        // the whole array; an empty one removes the key
        return stableStringify(value) === stableStringify(sync.known) ? null : { kind: "value", key, value: (value as unknown[]).length === 0 ? null : value }
      }
      const fields = changedFields(sync.known as Fields, value as Fields, defaults)
      return Object.keys(fields).length === 0 ? null : { kind: "fields", key, fields }
    },
    known: initialValue,
    sent: null,
    queued: null,
  }
  // a key created again (HMR) replaces the previous one
  syncs.set(key, sync)
  store.subscribe(() => {
    if (!applyingExternal) {
      changedLocally(sync)
    }
  })
  return store
}
