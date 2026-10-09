import { z } from "zod"
import { useStore } from "zustand"
import { createPersistentStore } from "./persistent-store"

// Validation schema with defaults
const AppSettingsSchema = z.object({
  primaryColor: z.string().default("green"),
  neutralColor: z.string().optional(),
  radius: z.number().optional(),
  globalUserBranchPrefix: z.string().optional(),
  autoSyncOnFocus: z.boolean().default(false),
})

export type AppSettings = z.infer<typeof AppSettingsSchema>

export const appSettingsStore = createPersistentStore(
  "appSettings",
  { schema: AppSettingsSchema, isArray: false, isMainOnly: false },
)

export function useAppSettings<T>(selector: (state: AppSettings) => T): T {
  return useStore(appSettingsStore, selector)
}

export function setPrimaryColor(primaryColor: string): void {
  appSettingsStore.setState({ primaryColor })
}

export function setAutoSyncOnFocus(autoSyncOnFocus: boolean): void {
  appSettingsStore.setState({ autoSyncOnFocus })
}

export function setGlobalUserBranchPrefix(globalUserBranchPrefix: string | undefined): void {
  appSettingsStore.setState({ globalUserBranchPrefix })
}
