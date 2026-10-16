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
}

const RESPONSE_TIMEOUT_MS = 10_000

/**
 * Sub-window side of the settings sync: the main window owns `settings.json`.
 * Local changes are sent to the main window as `store-set-request`;
 * changes made anywhere are received back as `store-snapshot-update`.
 */
export class SubWindowStoreSync {
  private requestCounter = 0
  private readonly pendingRequests = new Map<string, PendingRequest>()
  private readonly echoTracker = new EchoTracker()

  constructor(
    private readonly windowLabel: string,
    private readonly registry: ReadonlyMap<string, StoreRegistration>,
    private readonly rawSnapshot: Map<string, unknown>,
  ) {
    listen<{ key: string; value: unknown }>("store-snapshot-update", event => {
      this.handleSnapshotUpdate(event.payload.key, event.payload.value)
    }).catch((error: unknown) => {
      logError("Failed to listen to store-snapshot-update", error)
    })

    listen<StoreResponse>("store-response", event => {
      this.handleResponse(event.payload)
    }).catch((error: unknown) => {
      logError("Failed to listen to store-response", error)
    })
  }

  attach(registration: StoreRegistration): void {
    registration.subscribeLocal(() => {
      const value = registration.getValue()
      this.echoTracker.record(registration.key, stableStringify(value))
      this.setInMain(registration.key, value).catch((error: unknown) => {
        logError(`Failed to save ${registration.key} settings via main window`, error)
      })
    })
  }

  private handleSnapshotUpdate(key: string, value: unknown): void {
    const registration = this.registry.get(key)
    if (registration === undefined) {
      if (value === null || value === undefined) {
        this.rawSnapshot.delete(key)
      } else {
        this.rawSnapshot.set(key, value)
      }
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
    applyIncomingValue(registration, value)
  }

  private handleResponse(response: StoreResponse): void {
    const pending = this.pendingRequests.get(response.requestId)
    if (pending === undefined) {
      // a response for another sub-window
      return
    }
    this.pendingRequests.delete(response.requestId)
    clearTimeout(pending.timer)
    if (response.success) {
      pending.resolve()
    } else {
      pending.reject(new Error(response.error ?? "Store set failed"))
    }
  }

  private async setInMain(key: string, value: unknown): Promise<void> {
    // the window label keeps request ids unique across sub-windows, all of which receive every `store-response`
    const requestId = `store-set-${this.windowLabel}-${++this.requestCounter}`
    const response = new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pendingRequests.delete(requestId)
        reject(new Error(`No response from main window for ${key}`))
      }, RESPONSE_TIMEOUT_MS)
      this.pendingRequests.set(requestId, { resolve, reject, timer })
    })

    try {
      await emit("store-set-request", { requestId, key, value })
    } catch (error) {
      const pending = this.pendingRequests.get(requestId)
      if (pending !== undefined) {
        clearTimeout(pending.timer)
        this.pendingRequests.delete(requestId)
      }
      throw error
    }
    return response
  }
}
