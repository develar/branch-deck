import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { getErrorDetails } from "@/lib/errors"
import { aiSettingsStore } from "@/stores/settings/aiSettings"

export interface AIError {
  /** User-friendly error message */
  message: string
  /** Detailed error information (e.g., stack trace) */
  details: string
  /** When the error occurred (ms since epoch) */
  timestamp: number
}

export interface AISuggestionsState {
  /** Last failure of a branch name request; shown by the AI status indicator */
  aiError: AIError | null
}

export const aiSuggestionsStore = createStore<AISuggestionsState>(() => ({ aiError: null }))

export function useAISuggestionsStore<T>(selector: (state: AISuggestionsState) => T): T {
  return useStore(aiSuggestionsStore, selector)
}

export function setAIError(error: unknown): void {
  const message = error instanceof Error ? error.message : getErrorDetails(error)
  const details = error instanceof Error && error.stack ? error.stack : message
  aiSuggestionsStore.setState({ aiError: { message, details, timestamp: Date.now() } })
}

export function clearAIError(): void {
  if (aiSuggestionsStore.getState().aiError !== null) {
    aiSuggestionsStore.setState({ aiError: null })
  }
}

// a new answer forgets the last failure, whichever window changed it (Settings is a window of its own)
aiSettingsStore.subscribe((state, previous) => {
  if (state.aiMode !== previous.aiMode) {
    clearAIError()
  }
})
