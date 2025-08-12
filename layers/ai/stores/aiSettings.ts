import { z } from "zod"
import { createPersistentStore } from "~/utils/persistent-store"

// AI mode states
export type AIMode = "initial" | "enabled" | "disabled"

// Validation schema with defaults
const AISettingsSchema = z.object({
  aiMode: z.enum(["initial", "enabled", "disabled"]).default("initial"),
})

// Create the persistent store
export const aiSettingsStore = createPersistentStore(
  "ai",
  { schema: AISettingsSchema, isArray: false, isMainOnly: false },
)

// Export a composable for consistency with existing code
export const useAISettingsStore = () => aiSettingsStore

// Helper to check if AI is in initial state
export function isAIInitial(aiMode: AIMode): boolean {
  return aiMode === "initial"
}
