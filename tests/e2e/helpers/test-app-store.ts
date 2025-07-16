import type { IAppStore, ConflictViewerSettings, AppSettings, ModelSettings } from "~/utils/app-store"

/**
 * Test implementation of IAppStore that returns pre-configured values
 */
export class TestAppStore implements IAppStore {
  private recentPaths: string[] = []
  private selectedProject = ""
  private conflictViewerSettings: ConflictViewerSettings = {
    showConflictsOnly: true,
    viewMode: "diff",
    conflictDiffViewMode: "unified",
  }

  private appSettings: AppSettings = {
    primaryColor: "green",
    neutralColor: "slate",
    radius: 0.25,
  }

  private modelSettings: ModelSettings = {}

  // Allow setting the test repository path
  setTestRepository(path: string) {
    this.selectedProject = path
    this.recentPaths = [path]
  }

  async getRecentPaths(): Promise<string[]> {
    return this.recentPaths
  }

  async setRecentPaths(paths: string[]): Promise<void> {
    this.recentPaths = paths
  }

  async getSelectedProject(): Promise<string> {
    return this.selectedProject
  }

  async setSelectedProject(path: string): Promise<void> {
    this.selectedProject = path
  }

  async getConflictViewerSettings(): Promise<ConflictViewerSettings> {
    return this.conflictViewerSettings
  }

  async setConflictViewerSettings(settings: ConflictViewerSettings): Promise<void> {
    this.conflictViewerSettings = settings
  }

  async updateConflictViewerSetting<K extends keyof ConflictViewerSettings>(
    key: K,
    value: ConflictViewerSettings[K],
  ): Promise<void> {
    this.conflictViewerSettings[key] = value
  }

  async getAppSettings(): Promise<AppSettings> {
    return this.appSettings
  }

  async setAppSettings(settings: AppSettings): Promise<void> {
    this.appSettings = settings
  }

  async updateAppSetting<K extends keyof AppSettings>(
    key: K,
    value: AppSettings[K],
  ): Promise<void> {
    this.appSettings[key] = value
  }

  async getModelSettings(): Promise<ModelSettings> {
    return this.modelSettings
  }

  async setModelSettings(settings: ModelSettings): Promise<void> {
    this.modelSettings = settings
  }

  async updateModelSetting<K extends keyof ModelSettings>(
    key: K,
    value: ModelSettings[K],
  ): Promise<void> {
    this.modelSettings[key] = value
  }
}