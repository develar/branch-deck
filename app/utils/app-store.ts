import { LazyStore } from "@tauri-apps/plugin-store"
import { WebviewWindow } from "@tauri-apps/api/webviewWindow"
import { emit, listen } from "@tauri-apps/api/event"
import { SubWindowAppStore } from "./SubWindowAppStore"

export interface ConflictViewerSettings {
  showConflictsOnly?: boolean
  viewMode?: string
  conflictDiffViewMode?: "unified" | "split"
}

export interface AppSettings {
  primaryColor: string
  neutralColor: string
  radius: number
  globalUserBranchPrefix?: string
}

export interface ModelSettings {
  aiEnabled?: boolean
}

export interface ProjectMetadata {
  path: string
  cachedBranchPrefix?: string
  lastSyncTime?: number
  lastBranchCount?: number
  issueNavigationConfig?: IssueNavigationConfig
}

// Interface that both main and sub-window stores implement
export interface IAppStore {
  // Generic store methods
  get<T>(key: string): Promise<T | null>
  set(key: string, value: unknown): Promise<void>
}

// Main window implementation - direct store access
class MainAppStore implements IAppStore {
  public store: LazyStore // Public so handlers can access it
  private migrationChecked = false

  constructor() {
    this.store = new LazyStore("settings.json")
    // Run migration on initialization
    void this.ensureMigration()
  }

  // Generic store methods
  async get<T>(key: string): Promise<T | null> {
    return await this.store.get<T>(key) ?? null
  }

  async set(key: string, value: unknown): Promise<void> {
    if (value === null || value === undefined) {
      await this.store.delete(key)
    }
    else {
      await this.store.set(key, value)
    }
  }

  private async ensureMigration(): Promise<void> {
    if (this.migrationChecked) {
      return
    }
    this.migrationChecked = true

    // Migrate from old format to new format
    const oldPaths = await this.store.get<string[]>("recentsPaths")
    const oldSelectedProject = await this.store.get<string>("selectedProject")

    if (oldPaths && !await this.store.get<ProjectMetadata[]>("recentProjects")) {
      // Convert old paths to new project metadata
      const projects: ProjectMetadata[] = oldPaths.map(path => ({ path }))
      await this.store.set("recentProjects", projects)
    }

    if (oldSelectedProject && !await this.store.get<ProjectMetadata>("selectedProjectData")) {
      // Convert old selected project to new format
      await this.store.set("selectedProjectData", { path: oldSelectedProject })
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
  // Only initialize handlers if appStore is MainAppStore
  if (!(appStore instanceof MainAppStore)) {
    // Don't set up handlers in sub-windows
    return
  }

  // Use the existing appStore instance
  const mainStore = appStore

  // Handle store get requests from sub-windows
  void listen<{ requestId: string, key: string }>("store-get-request", async (event) => {
    try {
      const { requestId, key } = event.payload
      const data = await mainStore.store.get(key)

      await emit("store-response", {
        requestId,
        success: true,
        data,
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

  // Handle store set requests from sub-windows
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
