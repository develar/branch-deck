import { z } from "zod"
import { createAsyncPersistentStore } from "~/utils/persistent-store"

// Validation schema with defaults
const ConflictViewerSettingsSchema = z.object({
  showConflictsOnly: z.boolean().default(true),
  viewMode: z.string().default("diff"),
  conflictDiffViewMode: z.enum(["unified", "split"]).default("unified"),
})

// Create the persistent store promise
const conflictViewerStorePromise = createAsyncPersistentStore(
  "conflictViewerSettings",
  ConflictViewerSettingsSchema,
  "ConflictViewer",
)

// Cache the resolved store
let cachedStore: z.infer<typeof ConflictViewerSettingsSchema> | null = null

// Export a composable that returns the promise
export const useConflictViewerStore = async () => {
  if (cachedStore) {
    return cachedStore
  }

  cachedStore = await conflictViewerStorePromise

  // Debug logging in test mode
  if (process.env.NUXT_PUBLIC_TEST_MODE) {
    console.log("[ConflictViewerStore] Store accessed, current state:", cachedStore)
  }

  return cachedStore
}