import { z } from "zod"
import { useStore } from "zustand"
import { DEFAULT_PRIMARY_COLOR } from "@/lib/theme"
import { createPersistentStore } from "./persistent-store"

// Validation schema with defaults
const AppSettingsSchema = z.object({
  primaryColor: z.string().default(DEFAULT_PRIMARY_COLOR),
  neutralColor: z.string().optional(),
  radius: z.number().optional(),
  globalUserBranchPrefix: z.string().optional(),
  // on: a focus syncs only when what a sync reads changed (`syncIfChanged`)
  autoSyncOnFocus: z.boolean().default(true),
  // the zoom of the pages in every window (`useZoom`), read by the window builders too (`src-tauri/src/zoom.rs`)
  zoom: z.number().positive().default(1),
  // Settings → Notifications: the OS notifications in the background (`src/lib/os-notification.ts`)
  notifyReviewRequests: z.boolean().default(true),
  notifyPatronusMerges: z.boolean().default(true),
  notifyPatronusFailures: z.boolean().default(true),
  notifyApprovals: z.boolean().default(true),
  // Settings → Dock icon: what the badge counts (`src/lib/dock.ts`); Windows has no badge
  reviewBadge: z.boolean().default(true),
  attentionBadge: z.boolean().default(true),
  // "Assign to me" of the "Create pull request" dialog, as the user left it last
  createPullRequestAssignSelf: z.boolean().default(true),
})

export type AppSettings = z.infer<typeof AppSettingsSchema>

export const appSettingsStore = createPersistentStore("appSettings", { schema: AppSettingsSchema, isArray: false, isMainOnly: false })

export function useAppSettings<T>(selector: (state: AppSettings) => T): T {
  return useStore(appSettingsStore, selector)
}

export function setPrimaryColor(primaryColor: string): void {
  appSettingsStore.setState({ primaryColor })
}

export function setAutoSyncOnFocus(autoSyncOnFocus: boolean): void {
  appSettingsStore.setState({ autoSyncOnFocus })
}

export function setZoom(zoom: number): void {
  appSettingsStore.setState({ zoom })
}

/** A switch of the Settings dialog */
export type AppSettingsSwitch = "notifyReviewRequests" | "notifyPatronusMerges" | "notifyPatronusFailures" | "notifyApprovals" | "reviewBadge" | "attentionBadge"

export function setAppSettingsSwitch(name: AppSettingsSwitch, value: boolean): void {
  appSettingsStore.setState({ [name]: value })
}

export function setCreatePullRequestAssignSelf(createPullRequestAssignSelf: boolean): void {
  appSettingsStore.setState({ createPullRequestAssignSelf })
}

export function setGlobalUserBranchPrefix(globalUserBranchPrefix: string | undefined): void {
  appSettingsStore.setState({ globalUserBranchPrefix })
}
