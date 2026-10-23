import { z } from "zod"
import { useStore } from "zustand"
import type { StatsPeriod } from "@/lib/bindings"
import { STATS_PERIODS } from "@/lib/robot-stats"
import { SETTINGS_PAGES } from "@/lib/settings-pages"
import type { SettingsPageId } from "@/lib/settings-pages"
import { createPersistentStore } from "./persistent-store"

const AppSettingsSchema = z.object({
  globalUserBranchPrefix: z.string().optional(),
  // on: a focus syncs only when what a sync reads changed (`syncIfChanged`)
  autoSyncOnFocus: z.boolean().default(true),
  // the zoom of the pages in every window (`useZoom`), read by the window builders too (`src-tauri/src/zoom.rs`)
  zoom: z.number().positive().default(1),
  // the page of Settings shown last, where the window opens again
  settingsPage: z.enum(SETTINGS_PAGES).catch("general"),
  // Settings → Notifications: the OS notifications in the background (`src/lib/os-notification.ts`)
  notifyReviewRequests: z.boolean().default(true),
  notifyPatronusMerges: z.boolean().default(true),
  notifyPatronusFailures: z.boolean().default(true),
  notifyApprovals: z.boolean().default(true),
  // Settings → Notifications → Dock icon: what the badge counts (`src/lib/dock.ts`); Windows has no badge
  reviewBadge: z.boolean().default(true),
  attentionBadge: z.boolean().default(true),
  // Settings → Appearance → Title bar (Windows, Linux): the system title bar and menu bar instead of the page's, read by the backend at
  // startup (`stored_system_title_bar` in src-tauri/src/title_bar.rs)
  systemTitleBar: z.boolean().default(false),
  // "Assign to me" of the "Create pull request" dialog, as the user left it last
  createPullRequestAssignSelf: z.boolean().default(true),
  // the sidebar of a review (the commits and the files) beside the diffs; F and its button hide it, never the window
  reviewSidebar: z.boolean().default(true),
  // the Activity side panel of the main window (`src/stores/activity-panel.ts`); it shows where the window fits it
  activityPanel: z.boolean().default(false),
  // Settings → Patronus: the new API of Patronus, the progress of a running robot in one request and the
  // merge profile in the robot list (`new_api` of the Patronus commands; `bd` reads it from the settings file too)
  patronusNewApi: z.boolean().default(false),
  // the period of the Safe Push statistics (`StatsPeriod`), as the user chose it last in the Landing view
  statsPeriod: z.enum(STATS_PERIODS).catch("30d"),
  // the items of the Setup dot the user skipped: an update until another version, a reminder until its time (Unix s)
  setupSkips: z.array(z.object({ key: z.string(), until: z.number().nullable() })).catch([]),
})

export type AppSettings = z.infer<typeof AppSettingsSchema>

export const appSettingsStore = createPersistentStore("appSettings", { schema: AppSettingsSchema, isArray: false, isMainOnly: false })

export function useAppSettings<T>(selector: (state: AppSettings) => T): T {
  return useStore(appSettingsStore, selector)
}

export function setReviewSidebar(reviewSidebar: boolean): void {
  appSettingsStore.setState({ reviewSidebar })
}

export function setAutoSyncOnFocus(autoSyncOnFocus: boolean): void {
  appSettingsStore.setState({ autoSyncOnFocus })
}

export function setStatsPeriod(statsPeriod: StatsPeriod): void {
  appSettingsStore.setState({ statsPeriod })
}

export function setZoom(zoom: number): void {
  appSettingsStore.setState({ zoom })
}

/** A switch of a page of Settings */
export type AppSettingsSwitch =
  | "autoSyncOnFocus"
  | "notifyReviewRequests"
  | "notifyPatronusMerges"
  | "notifyPatronusFailures"
  | "notifyApprovals"
  | "reviewBadge"
  | "attentionBadge"
  | "patronusNewApi"

export function setAppSettingsSwitch(name: AppSettingsSwitch, value: boolean): void {
  appSettingsStore.setState({ [name]: value })
}

export function setSystemTitleBar(systemTitleBar: boolean): void {
  appSettingsStore.setState({ systemTitleBar })
}

export function setCreatePullRequestAssignSelf(createPullRequestAssignSelf: boolean): void {
  appSettingsStore.setState({ createPullRequestAssignSelf })
}

export function setGlobalUserBranchPrefix(globalUserBranchPrefix: string | undefined): void {
  appSettingsStore.setState({ globalUserBranchPrefix })
}

export function setSettingsPage(settingsPage: SettingsPageId): void {
  if (appSettingsStore.getState().settingsPage !== settingsPage) {
    appSettingsStore.setState({ settingsPage })
  }
}

/** Hides an item of the Setup dot (`SetupEntry.key`): for good (another version is another key) or until `until` */
export function skipSetupItem(key: string, until: number | null): void {
  const now = Date.now() / 1000
  const kept = appSettingsStore.getState().setupSkips.filter(skip => skip.key !== key && (skip.until === null || skip.until > now))
  appSettingsStore.setState({ setupSkips: [...kept, { key, until }] })
}
