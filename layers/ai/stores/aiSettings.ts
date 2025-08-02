import { z } from "zod"
import { createAsyncPersistentStore } from "~/utils/persistent-store"

// AI mode states
export type AIMode = "initial" | "enabled" | "disabled"

// Validation schema with defaults
const AISettingsSchema = z.object({
  aiMode: z.enum(["initial", "enabled", "disabled"]).default("initial"),
})

// Create the persistent store promise
const aiSettingsStorePromise = createAsyncPersistentStore(
  "modelSettings",
  AISettingsSchema,
  "AI",
)

// Cache the resolved store
let cachedStore: z.infer<typeof AISettingsSchema> | null = null

// Export a composable that returns the promise
export const useAISettingsStore = async () => {
  if (cachedStore) {
    return cachedStore
  }

  cachedStore = await aiSettingsStorePromise
  return cachedStore
}

// Helper to check if AI is in initial state
export function isAIInitial(aiMode: AIMode): boolean {
  return aiMode === "initial"
}
