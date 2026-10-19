import { z } from "zod"
import { useStore } from "zustand"
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
