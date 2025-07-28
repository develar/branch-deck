import { emit, listen } from "@tauri-apps/api/event"
import type { IAppStore } from "./app-store"

// Sub-window implementation - proxy to main window
export class SubWindowAppStore implements IAppStore {
  private requestCounter = 0

  // Generic store methods
  async get<T>(key: string): Promise<T | null> {
    return await this.getFromMain<T>(key)
  }

  async set(key: string, value: unknown): Promise<void> {
    await this.setInMain(key, value)
  }

  // Proxy method to get data from main window
  private async getFromMain<T>(key: string): Promise<T> {
    const requestId = `store-get-${++this.requestCounter}`

    // Set up listener for response
    const responsePromise = new Promise<T>((resolve, reject) => {
      const unlisten = listen<{ requestId: string, success: boolean, data?: T, error?: string }>(
        "store-response",
        (event) => {
          if (event.payload.requestId === requestId) {
            unlisten.then(fn => fn())
            if (event.payload.success) {
              resolve(event.payload.data as T)
            }
            else {
              reject(new Error(event.payload.error || "Store get failed"))
            }
          }
        },
      )
    })

    // Send request to main window
    await emit("store-get-request", { requestId, key })

    return responsePromise
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