import { LazyStore } from "@tauri-apps/plugin-store"
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

export class AppStore {
  private store: LazyStore

  constructor() {
    this.store = new LazyStore("settings.json")
  }

  // Recent paths methods
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

  // Conflict viewer settings
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

  // Update individual conflict viewer settings
  async updateConflictViewerSetting<K extends keyof ConflictViewerSettings>(
    key: K, 
    value: ConflictViewerSettings[K]
  ): Promise<void> {
    const current = await this.getConflictViewerSettings()
    current[key] = value
    await this.setConflictViewerSettings(current)
  }

  // App settings
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

// Create a singleton instance
export const appStore = new AppStore()

// Injection key for Vue
export const appStoreKey: InjectionKey<AppStore> = Symbol('appStore')