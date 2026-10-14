import { toast } from "@/components/ui/toast"
import { getErrorDetails } from "@/lib/errors"
import { cancelDownload, clearAIError, modelStore, useModelStore } from "@/stores/model"
import { aiSettingsStore, setAiMode, useAISettings } from "@/stores/settings/aiSettings"
import type { AIMode } from "@/stores/settings/aiSettings"

/** Stable toast id to prevent duplicate notifications */
const ERROR_TOAST_ID = "ai-mode-error"

/**
 * Changes the AI mode. Disabling while the model downloads only cancels the download:
 * the `Cancelled` event switches the mode to `disabled`.
 * Enabling doesn't download by itself: the next suggestion request reports a missing model and starts the download.
 */
export async function changeAIMode(value: AIMode): Promise<void> {
  try {
    if (value === "disabled" && modelStore.getState().isDownloading) {
      await cancelDownload()
      return
    }

    setAiMode(value)
    clearAIError()
  }
  catch (error) {
    toast.close(ERROR_TOAST_ID)
    toast.add({
      id: ERROR_TOAST_ID,
      type: "error",
      title: "Error",
      description: error instanceof Error ? error.message : getErrorDetails(error) || "Failed to change AI settings",
    })
  }
}

/** Flips between `enabled` and `disabled` (from `initial` it enables). */
export function toggleAIMode(): void {
  void changeAIMode(aiSettingsStore.getState().aiMode === "enabled" ? "disabled" : "enabled")
}

/** Classes of the sparkles status icon */
function getStatusIconClassName(aiMode: AIMode, isDownloading: boolean, hasError: boolean): string {
  // error state takes priority
  if (hasError) {
    return "text-destructive"
  }
  if (isDownloading) {
    return "text-primary animate-spin"
  }
  switch (aiMode) {
    case "enabled":
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
  const isDownloading = useModelStore(state => state.isDownloading)
  const aiError = useModelStore(state => state.aiError)

  return {
    aiMode,
    /** Whether the user hasn't made a choice yet */
    isInitial: aiMode === "initial",
    statusIconClassName: getStatusIconClassName(aiMode, isDownloading, aiError !== null),
    aiError,
    setAIMode: changeAIMode,
    toggleAIMode,
  }
}
