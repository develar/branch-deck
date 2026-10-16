import { z } from "zod"
import { useStore } from "zustand"
import { createPersistentStore } from "./persistent-store"

// Validation schema with defaults
const ConflictViewerSettingsSchema = z.object({
  conflictDiffViewMode: z.enum(["unified", "split"]).default("unified"),
})

export type ConflictViewerSettings = z.infer<typeof ConflictViewerSettingsSchema>
export type ConflictDiffViewMode = ConflictViewerSettings["conflictDiffViewMode"]

export const conflictViewerStore = createPersistentStore("conflictViewer", { schema: ConflictViewerSettingsSchema, isArray: false, isMainOnly: false })

export function useConflictViewerSettings<T>(selector: (state: ConflictViewerSettings) => T): T {
  return useStore(conflictViewerStore, selector)
}

export function setConflictDiffViewMode(conflictDiffViewMode: ConflictDiffViewMode): void {
  conflictViewerStore.setState({ conflictDiffViewMode })
}
