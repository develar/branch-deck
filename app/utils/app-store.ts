import { LazyStore } from "@tauri-apps/plugin-store"
import { WebviewWindow } from "@tauri-apps/api/webviewWindow"
import { emit, listen } from "@tauri-apps/api/event"
import type { InjectionKey } from "vue"

export interface ConflictViewerSettings {
  showConflictsOnly: boolean
  viewMode: string
  conflictDiffViewMode: 'unified' | 'split'
}

export interface AppSettings {
  primaryColor: string
  neutralColor: string
  radius: number
}

// Interface that both main and sub-window stores implement
export interface IAppStore {
  getRecentPaths(): Promise<string[]>
  setRecentPaths(paths: string[]): Promise<void>
  getSelectedProject(): Promise<string>
  setSelectedProject(path: string): Promise<void>
  getConflictViewerSettings(): Promise<ConflictViewerSettings>
  setConflictViewerSettings(settings: ConflictViewerSettings): Promise<void>
  updateConflictViewerSetting<K extends keyof ConflictViewerSettings>(key: K, value: ConflictViewerSettings[K]): Promise<void>
  getAppSettings(): Promise<AppSettings>
  setAppSettings(settings: AppSettings): Promise<void>
  updateAppSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]): Promise<void>
}

// Main window implementation - direct store access
class MainAppStore implements IAppStore {
  public store: LazyStore // Public so handlers can access it

  constructor() {
    this.store = new LazyStore("settings.json")
  }

  async getRecentPaths(): Promise<string[]> {
    return await this.store.get<string[]>("recentsPaths") ?? []
  }

  async setRecentPaths(paths: string[]): Promise<void> {
    await this.store.set("recentsPaths", paths)
  }

  async getSelectedProject(): Promise<string> {
    return await this.store.get<string>("selectedProject") ?? ""
  }

  async setSelectedProject(path: string): Promise<void> {
    await this.store.set("selectedProject", path)
  }

  async getConflictViewerSettings(): Promise<ConflictViewerSettings> {
    const settings = await this.store.get<ConflictViewerSettings>("conflictViewerSettings")
    return settings ?? {
      showConflictsOnly: true,
      viewMode: 'diff',
      conflictDiffViewMode: 'unified'
    }
  }

  async setConflictViewerSettings(settings: ConflictViewerSettings): Promise<void> {
    await this.store.set("conflictViewerSettings", settings)
  }

  async updateConflictViewerSetting<K extends keyof ConflictViewerSettings>(
    key: K, 
    value: ConflictViewerSettings[K]
  ): Promise<void> {
    const current = await this.getConflictViewerSettings()
    current[key] = value
    await this.setConflictViewerSettings(current)
  }

  async getAppSettings(): Promise<AppSettings> {
    const settings = await this.store.get<AppSettings>("appSettings")
    return settings ?? {
      primaryColor: 'blue',
      neutralColor: 'slate',
      radius: 0.25
    }
  }

  async setAppSettings(settings: AppSettings): Promise<void> {
    await this.store.set("appSettings", settings)
  }

  async updateAppSetting<K extends keyof AppSettings>(
    key: K,
    value: AppSettings[K]
  ): Promise<void> {
    const current = await this.getAppSettings()
    current[key] = value
    await this.setAppSettings(current)
  }
}

// Sub-window implementation - proxy to main window
class SubWindowAppStore implements IAppStore {
  private requestCounter = 0

  // Proxy method to get data from main window
  private async getFromMain<T>(key: string): Promise<T> {
    const requestId = `store-get-${++this.requestCounter}`
    
    // Set up listener for response
    const responsePromise = new Promise<T>((resolve, reject) => {
      const unlisten = listen<{ requestId: string; success: boolean; data?: T; error?: string }>(
        'store-response',
        (event) => {
          if (event.payload.requestId === requestId) {
            unlisten.then(fn => fn())
            if (event.payload.success) {
              resolve(event.payload.data as T)
            } else {
              reject(new Error(event.payload.error || 'Store get failed'))
            }
          }
        }
      )
    })

    // Send request to main window
    await emit('store-get-request', { requestId, key })
    
    return responsePromise
  }

  // Proxy method to set data via main window
  private async setInMain(key: string, value: unknown): Promise<void> {
    const requestId = `store-set-${++this.requestCounter}`
    
    // Set up listener for response
    const responsePromise = new Promise<void>((resolve, reject) => {
      const unlisten = listen<{ requestId: string; success: boolean; error?: string }>(
        'store-response',
        (event) => {
          if (event.payload.requestId === requestId) {
            unlisten.then(fn => fn())
            if (event.payload.success) {
              resolve()
            } else {
              reject(new Error(event.payload.error || 'Store set failed'))
            }
          }
        }
      )
    })

    // Send request to main window
    await emit('store-set-request', { requestId, key, value })
    
    return responsePromise
  }

  async getRecentPaths(): Promise<string[]> {
    return await this.getFromMain<string[]>("recentsPaths") ?? []
  }

  async setRecentPaths(paths: string[]): Promise<void> {
    await this.setInMain("recentsPaths", paths)
  }

  async getSelectedProject(): Promise<string> {
    return await this.getFromMain<string>("selectedProject") ?? ""
  }

  async setSelectedProject(path: string): Promise<void> {
    await this.setInMain("selectedProject", path)
  }

  async getConflictViewerSettings(): Promise<ConflictViewerSettings> {
    const settings = await this.getFromMain<ConflictViewerSettings>("conflictViewerSettings")
    return settings ?? {
      showConflictsOnly: true,
      viewMode: 'diff',
      conflictDiffViewMode: 'unified'
    }
  }

  async setConflictViewerSettings(settings: ConflictViewerSettings): Promise<void> {
    await this.setInMain("conflictViewerSettings", settings)
  }

  async updateConflictViewerSetting<K extends keyof ConflictViewerSettings>(
    key: K, 
    value: ConflictViewerSettings[K]
  ): Promise<void> {
    const current = await this.getConflictViewerSettings()
    current[key] = value
    await this.setConflictViewerSettings(current)
  }

  async getAppSettings(): Promise<AppSettings> {
    const settings = await this.getFromMain<AppSettings>("appSettings")
    return settings ?? {
      primaryColor: 'blue',
      neutralColor: 'slate',
      radius: 0.25
    }
  }

  async setAppSettings(settings: AppSettings): Promise<void> {
    await this.setInMain("appSettings", settings)
  }

  async updateAppSetting<K extends keyof AppSettings>(
    key: K,
    value: AppSettings[K]
  ): Promise<void> {
    const current = await this.getAppSettings()
    current[key] = value
    await this.setAppSettings(current)
  }
}

// Factory function to create the appropriate store implementation
function createAppStore(): IAppStore {
  try {
    const currentWindow = WebviewWindow.getCurrent()
    const label = currentWindow.label
    if (label === 'main') {
      return new MainAppStore()
    } else {
      return new SubWindowAppStore()
    }
  } catch {
    // If we can't determine the window, assume we're in main
    return new MainAppStore()
  }
}

// Store request handlers for main window
export function initializeStoreHandlers() {
  // Only initialize handlers if appStore is MainAppStore
  if (!(appStore instanceof MainAppStore)) {
    return // Don't set up handlers in sub-windows
  }

  // Use the existing appStore instance
  const mainStore = appStore

  // Handle store get requests from sub-windows
  void listen<{ requestId: string; key: string }>('store-get-request', async (event) => {
    try {
      const { requestId, key } = event.payload
      const data = await mainStore.store.get(key)
      
      await emit('store-response', {
        requestId,
        success: true,
        data
      })
    } catch (error) {
      await emit('store-response', {
        requestId: event.payload.requestId,
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error'
      })
    }
  })

  // Handle store set requests from sub-windows
  void listen<{ requestId: string; key: string; value: unknown }>('store-set-request', async (event) => {
    try {
      const { requestId, key, value } = event.payload
      await mainStore.store.set(key, value)
      
      await emit('store-response', {
        requestId,
        success: true
      })
    } catch (error) {
      await emit('store-response', {
        requestId: event.payload.requestId,
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error'
      })
    }
  })
}

// Create a singleton instance using factory
export const appStore = createAppStore()

// Injection key for Vue
export const appStoreKey: InjectionKey<IAppStore> = Symbol('appStore')