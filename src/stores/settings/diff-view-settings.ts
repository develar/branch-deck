import { z } from "zod"
import { useStore } from "zustand"
import { createPersistentStore } from "./persistent-store"

// Unified or Split, one choice for every diff view (`DiffViewToggle`), persisted under its older name "conflictViewer"
const ConflictViewerSettingsSchema = z.object({
  conflictDiffViewMode: z.enum(["unified", "split"]).default("unified"),
})

type ConflictViewerSettings = z.infer<typeof ConflictViewerSettingsSchema>
export type ConflictDiffViewMode = ConflictViewerSettings["conflictDiffViewMode"]

const conflictViewerStore = createPersistentStore("conflictViewer", { schema: ConflictViewerSettingsSchema, isArray: false, isMainOnly: false })

export function useConflictViewerSettings<T>(selector: (state: ConflictViewerSettings) => T): T {
  return useStore(conflictViewerStore, selector)
}

export function setConflictDiffViewMode(conflictDiffViewMode: ConflictDiffViewMode): void {
  conflictViewerStore.setState({ conflictDiffViewMode })
}
