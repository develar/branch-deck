import { useAISuggestionsStore } from "@/stores/ai-suggestions"
import { aiSettingsStore, setAiMode, useAISettings } from "@/stores/settings/aiSettings"
import type { AIMode } from "@/stores/settings/aiSettings"

/** Changes the AI mode; a new answer forgets the last failure (`ai-suggestions.ts`) */
export function changeAIMode(value: AIMode): void {
  setAiMode(value)
}

/** Flips between `claude` and `disabled` (from `initial` it enables) */
export function toggleAIMode(): void {
  changeAIMode(aiSettingsStore.getState().aiMode === "claude" ? "disabled" : "claude")
}

/** Classes of the sparkles status icon */
function getStatusIconClassName(aiMode: AIMode, hasError: boolean): string {
  if (hasError) {
    return "text-destructive"
  }
  switch (aiMode) {
    case "claude":
      return "text-primary"
    case "disabled":
      return "text-muted-foreground"
    case "initial":
      return "text-muted-foreground group-hover/button:text-primary"
  }
}

/** AI mode, status icon classes and error state for the AI controls. */
export function useAIToggle() {
  const aiMode = useAISettings(state => state.aiMode)
  const aiError = useAISuggestionsStore(state => state.aiError)

  return {
    aiMode,
    /** Whether the user hasn't made a choice yet */
    isInitial: aiMode === "initial",
    statusIconClassName: getStatusIconClassName(aiMode, aiError !== null),
    aiError,
    setAIMode: changeAIMode,
    toggleAIMode,
  }
}
