import { z } from "zod"
import { createPersistentStore } from "../utils/persistent-store"

// Validation schema with defaults
const ConflictViewerSettingsSchema = z.object({
  showConflictsOnly: z.boolean().default(true),
  viewMode: z.string().default("diff"),
  conflictDiffViewMode: z.enum(["unified", "split"]).default("unified"),
})

// Create the persistent store
export const conflictViewerStore = createPersistentStore(
  "conflictViewer",
  { schema: ConflictViewerSettingsSchema, isArray: false, isMainOnly: false },
)

// Export a composable for consistency with existing code
export const useConflictViewerStore = () => conflictViewerStore
