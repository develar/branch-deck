import { z } from "zod"
import { useStore } from "zustand"
import { createPersistentStore } from "./persistent-store"

// AI mode states
export type AIMode = "initial" | "enabled" | "disabled"

// Validation schema with defaults
const AISettingsSchema = z.object({
  aiMode: z.enum(["initial", "enabled", "disabled"]).default("initial"),
})

export type AISettings = z.infer<typeof AISettingsSchema>

export const aiSettingsStore = createPersistentStore(
  "ai",
  { schema: AISettingsSchema, isArray: false, isMainOnly: false },
)

export function useAISettings<T>(selector: (state: AISettings) => T): T {
  return useStore(aiSettingsStore, selector)
}

export function setAiMode(aiMode: AIMode): void {
  aiSettingsStore.setState({ aiMode })
}

// Helper to check if AI is in initial state
export function isAIInitial(aiMode: AIMode): boolean {
  return aiMode === "initial"
}
