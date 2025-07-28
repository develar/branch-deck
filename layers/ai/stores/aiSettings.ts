import { z } from "zod"
import { createPersistentStore } from "~/utils/persistent-store"

// Validation schema with defaults
const AISettingsSchema = z.object({
  aiEnabled: z.boolean().default(false),
})

// Create the persistent store
export const aiSettingsStore = createPersistentStore(
  "modelSettings",
  AISettingsSchema,
  "AI",
)

// Export a composable for consistency with existing code
export const useAISettingsStore = () => aiSettingsStore