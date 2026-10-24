import { z } from "zod"
import { useStore } from "zustand"
import { APP_SETTINGS_DEFAULTS } from "@/lib/bindings"
import type { AppSettings as RustAppSettings, StatsPeriod } from "@/lib/bindings"
import { STATS_PERIODS } from "@/lib/robot-stats"
import { SETTINGS_PAGES } from "@/lib/settings-pages"
import type { SettingsPageId } from "@/lib/settings-pages"
import { createPersistentStore, snapshotValueOf } from "./persistent-store"

/**
 * The fields Rust reads (`AppSettings` of `domain-types`, the type of the bindings): their defaults are Rust's
 * (`APP_SETTINGS_DEFAULTS`, which the bindings export from `AppSettings::default`), so the page, the backend and `bd`
 * never disagree on one, and a value back at its default goes to the writer as no value; Zod only checks the types.
 * The `satisfies` fails the build when Rust gains or loses a field
 */
const RustFields = {
  // General → Branch prefix: the prefix of a repository whose git config sets none (`resolve_branch_prefix`)
  globalUserBranchPrefix: z.string().optional(),
  // on: a focus syncs only when what a sync reads changed; the session reads it (`SessionHost::syncs_on_focus`)
  autoSyncOnFocus: z.boolean().default(APP_SETTINGS_DEFAULTS.autoSyncOnFocus),
  // the zoom of the pages in every window (`useZoom`), read by the window builders too (`src-tauri/src/zoom.rs`)
  zoom: z.number().positive().default(APP_SETTINGS_DEFAULTS.zoom),
  // Settings → Appearance → Title bar (Windows, Linux): the system title bar and menu bar instead of the page's, read by the backend at
  // startup (`init_title_bar` in src-tauri/src/title_bar.rs)
  systemTitleBar: z.boolean().default(APP_SETTINGS_DEFAULTS.systemTitleBar),
  // Settings → Patronus: the new API of Patronus, the progress of a running robot in one request and the
  // merge profile in the robot list (`new_api` of the Patronus commands; `bd` reads it from the settings file too)
  patronusNewApi: z.boolean().default(APP_SETTINGS_DEFAULTS.patronusNewApi),
} satisfies { [K in keyof RustAppSettings]-?: z.ZodType }

/** `appSettings`: the fields Rust reads, then the fields only the page reads, with the page's defaults */
const AppSettingsSchema = z.object({
  ...RustFields,
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
  // "Assign to me" of the "Create pull request" dialog, as the user left it last
  createPullRequestAssignSelf: z.boolean().default(true),
  // the sidebar of a review (the commits and the files) beside the diffs; F and its button hide it, never the window
  reviewSidebar: z.boolean().default(true),
  // the Activity side panel of the main window (`src/stores/activity-panel.ts`); it shows where the window fits it
  activityPanel: z.boolean().default(false),
  // the period of the Safe Push statistics (`StatsPeriod`), as the user chose it last in the Landing view
  statsPeriod: z.enum(STATS_PERIODS).catch("30d"),
  // the items of the Setup dot the user skipped: an update until another version, a reminder until its time (Unix s)
  setupSkips: z.array(z.object({ key: z.string(), until: z.number().nullable() })).catch([]),
})

export type AppSettings = z.infer<typeof AppSettingsSchema>

export const appSettingsStore = createPersistentStore("appSettings", { schema: AppSettingsSchema, isArray: false, isMainOnly: false })

/**
 * The zoom the window was built with: the builder zooms the webview to the level of the snapshot its page starts from
 * (`apply_zoom` in src-tauri/src/zoom.rs), so `useZoom` zooms the page whenever the level differs from it, also when a
 * change made while the page loaded arrived before the page rendered
 */
export const zoomAtStart: number = RustFields.zoom.catch(APP_SETTINGS_DEFAULTS.zoom).parse((snapshotValueOf("appSettings") as { zoom?: unknown } | undefined)?.zoom)

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
