import { useModelState } from "./modelProvider"
import type { AIMode } from "../stores/aiSettings"

/** Stable toast IDs to prevent duplicate notifications */
const TOAST_IDS = {
  ERROR: "ai-mode-error",
} as const

/**
 * UI display properties for AI status indicator
 */
interface AIStatusDisplay {
  /** Tooltip text to display on hover */
  tooltip: string
  /** CSS classes for the status icon */
  iconClass: string
  /** Whether the AI is in an error state */
  hasError?: boolean
}

/**
 * AI error information
 */
interface AIError {
  /** User-friendly error message */
  message: string
  /** Detailed error information (e.g., stack trace) */
  details: string
  /** When the error occurred */
  timestamp: Date
}

/**
 * Composable for managing AI toggle state and UI interactions.
 * Handles enabling/disabling AI, error states, and status display.
 *
 * Key features:
 * - Reactive AI enabled/disabled state
 * - Computed status display properties for UI
 * - Error state management with user notifications
 * - Automatic download triggering when enabling AI
 *
 * @returns {Promise<Object>} Object containing:
 *   - aiMode: Computed ref for AI mode ("initial" | "enabled" | "disabled")
 *   - aiStatus: Readonly computed status display properties
 *   - aiError: Readonly ref of current error state
 *   - isInitial: Readonly computed whether AI is in initial state
 *   - setAIError: Function to set error state
 *   - clearAIError: Function to clear error state
 *
 * @example
 * const { aiMode } = useAIToggle()
 * // Set AI mode directly
 * aiMode.value = "enabled"
 */
export function useAIToggle() {
  const modelState = useModelState()
  const toast = useToast()
  const aiSettingsStore = useAISettingsStore()

  /** Current AI error state */
  const aiError = ref<AIError | null>(null)

  /** AI mode getter and setter */
  const aiMode = computed({
    get() {
      return aiSettingsStore.aiMode
    },
    async set(value: AIMode) {
      try {
        // Only cancel download when switching to disabled
        if (value === "disabled" && modelState.isDownloading.value) {
          await modelState.cancelDownload()
          // State will be updated by the Cancelled event from backend
          return
        }

        // Set the new mode
        aiSettingsStore.aiMode = value
        clearAIError() // Clear any previous errors
      }
      catch (error) {
        toast.remove(TOAST_IDS.ERROR)
        toast.add({
          id: TOAST_IDS.ERROR,
          title: "Error",
          description: error instanceof Error ? error.message : "Failed to change AI settings",
          color: "error",
        })
      }
    },
  })

  /** Computed AI status for UI display */
  const aiStatus = computed<AIStatusDisplay>(() => {
    // Error state - takes priority
    if (aiError.value) {
      return {
        tooltip: "AI malfunction - click for details",
        iconClass: "text-error",
        hasError: true,
      }
    }

    // Downloading state
    if (modelState.isDownloading.value) {
      return {
        tooltip: "Downloading AI model...",
        iconClass: "text-primary animate-spin",
      }
    }

    // User preference states
    switch (aiSettingsStore.aiMode) {
      case "enabled":
        return {
          tooltip: "AI suggestions enabled",
          iconClass: "text-primary",
        }
      case "disabled":
        return {
          tooltip: "AI suggestions disabled",
          iconClass: "text-muted",
        }
      case "initial":
        return {
          tooltip: "Enable AI suggestions - click to get started",
          iconClass: "text-muted hover:text-primary",
        }
      default:
        return {
          tooltip: "AI suggestions",
          iconClass: "text-muted",
        }
    }
  })

  /**
   * Sets an AI error state with message and details.
   *
   * @param error - Error object or message string
   */
  function setAIError(error: string | Error) {
    const errorMessage = error instanceof Error ? error.message : error
    const errorDetails = error instanceof Error && error.stack ? error.stack : errorMessage

    aiError.value = {
      message: errorMessage,
      details: errorDetails,
      timestamp: new Date(),
    }
  }

  /**
   * Clears the current AI error state.
   */
  function clearAIError() {
    aiError.value = null
  }

  /** Whether AI is in initial state (user hasn't made a choice yet) */
  const isInitial = computed(() => aiSettingsStore.aiMode === "initial")

  return {
    aiMode,
    aiStatus: readonly(aiStatus),
    aiError: readonly(aiError),
    isInitial,
    setAIError,
    clearAIError,
  }
}
