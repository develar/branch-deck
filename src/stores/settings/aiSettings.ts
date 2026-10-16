import { z } from "zod"
import { useStore } from "zustand"
import { createPersistentStore } from "./persistent-store"

/**
 * Whether branch names are suggested: `initial` until the user answers the prompt, `claude` once the user agreed that
 * the commit messages and file names go to Claude through their Claude Code.
 */
export type AIMode = "initial" | "claude" | "disabled"

/** What `claude` consents to, said by the prompt in Move to… and by the switch in Settings */
export const aiConsentNote = "Sends the commit messages and changed file names to Claude (Haiku) through your Claude Code"

export const AISettingsSchema = z.object({
  // a stored `enabled` was the consent to the on-device model that `claude` replaced: the prompt asks again
  aiMode: z.enum(["initial", "claude", "disabled"]).catch("initial"),
})

export type AISettings = z.infer<typeof AISettingsSchema>

export const aiSettingsStore = createPersistentStore("ai", { schema: AISettingsSchema, isArray: false, isMainOnly: false })

export function useAISettings<T>(selector: (state: AISettings) => T): T {
  return useStore(aiSettingsStore, selector)
}

export function setAiMode(aiMode: AIMode): void {
  aiSettingsStore.setState({ aiMode })
}
