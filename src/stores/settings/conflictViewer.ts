import { z } from "zod"
import { useStore } from "zustand"
import { createPersistentStore } from "./persistent-store"

// Validation schema with defaults
const ConflictViewerSettingsSchema = z.object({
  showConflictsOnly: z.boolean().default(true),
  viewMode: z.string().default("diff"),
  conflictDiffViewMode: z.enum(["unified", "split"]).default("unified"),
})

export type ConflictViewerSettings = z.infer<typeof ConflictViewerSettingsSchema>
export type ConflictDiffViewMode = ConflictViewerSettings["conflictDiffViewMode"]

export const conflictViewerStore = createPersistentStore("conflictViewer", { schema: ConflictViewerSettingsSchema, isArray: false, isMainOnly: false })

export function useConflictViewerSettings<T>(selector: (state: ConflictViewerSettings) => T): T {
  return useStore(conflictViewerStore, selector)
}

export function setShowConflictsOnly(showConflictsOnly: boolean): void {
  conflictViewerStore.setState({ showConflictsOnly })
}

export function setConflictViewMode(viewMode: string): void {
  conflictViewerStore.setState({ viewMode })
}

export function setConflictDiffViewMode(conflictDiffViewMode: ConflictDiffViewMode): void {
  conflictViewerStore.setState({ conflictDiffViewMode })
}
