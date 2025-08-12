import { z } from "zod"
import { createPersistentStore } from "~/utils/persistent-store"

// Validation schema with defaults
const AppSettingsSchema = z.object({
  primaryColor: z.string().default("green"),
  neutralColor: z.string().optional(),
  radius: z.number().optional(),
  globalUserBranchPrefix: z.string().optional(),
})

// Create the persistent store
export const appSettingsStore = createPersistentStore(
  "appSettings",
  { schema: AppSettingsSchema, isArray: false, isMainOnly: false },
)

// Export a composable for consistency with existing code
export const useAppSettingsStore = () => appSettingsStore
