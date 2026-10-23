import { emit, listen } from "@tauri-apps/api/event"
import { logError } from "@/lib/errors"
import { applyIncomingValue, EchoTracker, stableStringify } from "./sync-utils"
import type { StoreRegistration } from "./sync-utils"

interface StoreResponse {
  requestId: string
  success: boolean
  error?: string
}

interface PendingRequest {
  resolve: () => void
  reject: (error: Error) => void
  timer: ReturnType<typeof setTimeout>
  key: string
  fields: readonly string[] | undefined
}

/** The settings of every window as the main window has them, for the sub-window that asked (`store-snapshot-request`) */
export interface StoreSnapshot {
  label: string
  snapshot: Record<string, unknown>
}

/** The fields of an object store whose values differ between `before` and `after` */
export function changedFields(before: Record<string, unknown>, after: Record<string, unknown>): string[] {
  const keys = new Set([...Object.keys(before), ...Object.keys(after)])
  return [...keys].filter(key => stableStringify(before[key]) !== stableStringify(after[key]))
}

const RESPONSE_TIMEOUT_MS = 10_000

/**
 * Sub-window side of the settings sync: the main window owns `settings.json`.
 * Local changes are sent to the main window as `store-set-request`, with the fields of an object store they changed,
 * which the main window applies to its own value: a field the main window changed meanwhile stays. Changes made
 * anywhere are received back as `store-snapshot-update`, and an update keeps the fields of this window's writes the
 * main window has not answered yet: an object store records no echo, because the main window's broadcast holds its own
 * fields too. Once it listens, the window asks for the settings again (`store-snapshot-request`): a change made while
 * its page loaded reached no listener.
 */
export class SubWindowStoreSync {
  private requestCounter = 0
  private readonly pendingRequests = new Map<string, PendingRequest>()
  private readonly echoTracker = new EchoTracker()
  // the value of each object store as this window last knew it: a local change sends the fields that differ from it
  private readonly known = new Map<string, Record<string, unknown>>()
  // the fields of each key in writes the main window has not answered yet, with their count
  private readonly unanswered = new Map<string, Map<string, number>>()

  constructor(
    private readonly windowLabel: string,
    private readonly registry: ReadonlyMap<string, StoreRegistration>,
    private readonly rawSnapshot: Map<string, unknown>,
  ) {
    Promise.all([
      listen<{ key: string; value: unknown }>("store-snapshot-update", event => {
        this.handleSnapshotUpdate(event.payload.key, event.payload.value)
      }),
      listen<StoreResponse>("store-response", event => {
        this.handleResponse(event.payload)
      }),
      listen<StoreSnapshot>("store-snapshot", event => {
        if (event.payload.label === this.windowLabel) {
          for (const [key, value] of Object.entries(event.payload.snapshot)) {
            this.applyFromMain(key, value)
          }
        }
      }),
    ])
      .then(() => emit("store-snapshot-request", { label: this.windowLabel }))
      .catch((error: unknown) => {
        logError("Failed to listen to the settings of the main window", error)
      })
  }

  attach(registration: StoreRegistration): void {
    if (!registration.isArray) {
      this.known.set(registration.key, registration.getValue() as Record<string, unknown>)
    }
    registration.subscribeLocal(() => {
      const value = registration.getValue()
      let fields: string[] | undefined
      if (registration.isArray) {
        // the main window takes the whole array, so its broadcast is this value
        this.echoTracker.record(registration.key, stableStringify(value))
      } else {
        // no echo: the main window broadcasts these fields on its own value, which a recorded whole object would not
        // match, and the stale entry would later drop a real update equal to it; the unanswered fields guard the write
        const current = value as Record<string, unknown>
        fields = changedFields(this.known.get(registration.key) ?? {}, current)
        this.known.set(registration.key, current)
      }
      this.setInMain(registration.key, value, fields).catch((error: unknown) => {
        logError(`Failed to save ${registration.key} settings via main window`, error)
      })
    })
  }

  private handleSnapshotUpdate(key: string, value: unknown): void {
    const registration = this.registry.get(key)
    if (registration?.isArray !== true) {
      this.applyFromMain(key, value)
      return
    }

    // the main window persists only non-default fields, so compare the parsed (defaults-filled) value
    let serialized: string
    try {
      serialized = stableStringify(registration.parse(value))
    } catch {
      // invalid value; applyIncomingValue logs it
      applyIncomingValue(registration, value)
      return
    }
    if (this.echoTracker.consume(key, serialized)) {
      return
    }
    this.applyFromMain(key, value)
  }

  /** Applies a value of the main window, keeping the fields of this window's unanswered writes */
  private applyFromMain(key: string, value: unknown): void {
    const registration = this.registry.get(key)
    if (registration === undefined) {
      if (value === null || value === undefined) {
        this.rawSnapshot.delete(key)
      } else {
        this.rawSnapshot.set(key, value)
      }
      return
    }
    const fields = this.unanswered.get(key)
    let incoming = value
    if (!registration.isArray && fields !== undefined && fields.size > 0) {
      const local = registration.getValue() as Record<string, unknown>
      incoming = { ...(value as Record<string, unknown> | null), ...Object.fromEntries([...fields.keys()].map(field => [field, local[field]])) }
    }
    applyIncomingValue(registration, incoming)
    if (!registration.isArray) {
      this.known.set(key, registration.getValue() as Record<string, unknown>)
    }
  }

  private countUnanswered(key: string, fields: readonly string[] | undefined, delta: 1 | -1): void {
    if (fields === undefined) {
      return
    }
    let counts = this.unanswered.get(key)
    if (counts === undefined) {
      counts = new Map()
      this.unanswered.set(key, counts)
    }
    for (const field of fields) {
      const count = (counts.get(field) ?? 0) + delta
      if (count > 0) {
        counts.set(field, count)
      } else {
        counts.delete(field)
      }
    }
  }

  private handleResponse(response: StoreResponse): void {
    const pending = this.pendingRequests.get(response.requestId)
    if (pending === undefined) {
      // a response for another sub-window
      return
    }
    this.pendingRequests.delete(response.requestId)
    clearTimeout(pending.timer)
    this.countUnanswered(pending.key, pending.fields, -1)
    if (response.success) {
      pending.resolve()
    } else {
      pending.reject(new Error(response.error ?? "Store set failed"))
    }
  }

  private async setInMain(key: string, value: unknown, fields: readonly string[] | undefined): Promise<void> {
    // the window label keeps request ids unique across sub-windows, all of which receive every `store-response`
    const requestId = `store-set-${this.windowLabel}-${++this.requestCounter}`
    this.countUnanswered(key, fields, 1)
    const response = new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pendingRequests.delete(requestId)
        this.countUnanswered(key, fields, -1)
        reject(new Error(`No response from main window for ${key}`))
      }, RESPONSE_TIMEOUT_MS)
      this.pendingRequests.set(requestId, { resolve, reject, timer, key, fields })
    })

    try {
      await emit("store-set-request", { requestId, key, value, fields })
    } catch (error) {
      const pending = this.pendingRequests.get(requestId)
      if (pending !== undefined) {
        clearTimeout(pending.timer)
        this.pendingRequests.delete(requestId)
        this.countUnanswered(key, fields, -1)
      }
      throw error
    }
    return response
  }
}
