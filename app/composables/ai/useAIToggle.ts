import { useAIStatus } from "./useAIStatus"
import { appStore } from "~/utils/app-store"

interface AIStatusDisplay {
  tooltip: string
  iconClass: string
  hasError?: boolean
}

interface AIError {
  message: string
  details: string
  timestamp: Date
}

// Stable toast IDs to prevent duplicates
const TOAST_IDS = {
  DOWNLOAD_CANCEL: "ai-download-cancel",
  ERROR: "ai-toggle-error",
}

export function useAIToggle() {
  const { modelStatus, isDownloading, triggerDownload } = useAIStatus()
  const toast = useToast()

  // Track AI preference state
  const aiPreference = ref<boolean | null>(null)

  // Track AI error state
  const aiError = ref<AIError | null>(null)

  // Load AI preference on mount
  onMounted(async () => {
    const modelSettings = await appStore.getModelSettings()
    aiPreference.value = modelSettings.aiEnabled ?? null
  })

  // Computed property for switch v-model
  const aiEnabled = computed({
    get() {
      return modelStatus.value?.available && aiPreference.value !== false
    },
    set(_value: boolean) {
      toggleAI()
    },
  })

  // AI status display
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
    if (isDownloading.value) {
      return {
        tooltip: "Downloading AI model...",
        iconClass: "text-primary animate-spin",
      }
    }

    // Available and enabled
    if (modelStatus.value?.available && aiPreference.value !== false) {
      return {
        tooltip: "AI suggestions enabled",
        iconClass: "text-primary",
      }
    }

    // Disabled by user preference
    if (aiPreference.value === false) {
      return {
        tooltip: "AI suggestions disabled",
        iconClass: "text-muted",
      }
    }

    // Default: not available
    return {
      tooltip: "Enable AI suggestions - model will be downloaded",
      iconClass: "text-muted",
    }
  })

  // Toggle AI enable/disable
  async function toggleAI() {
    try {
      // Handle different states
      if (isDownloading.value) {
        // TODO: Implement download cancellation if needed
        toast.add({
          id: TOAST_IDS.DOWNLOAD_CANCEL,
          title: "Download Cancellation",
          description: "Download cancellation is not yet implemented",
          color: "neutral",
          duration: 3000,
        })
        return
      }

      if (modelStatus.value?.available && aiPreference.value !== false) {
        // Disable AI
        await appStore.updateModelSetting("aiEnabled", false)
        aiPreference.value = false
        clearAIError() // Clear any previous errors
      }
      else {
        // Enable AI
        await appStore.updateModelSetting("aiEnabled", true)
        aiPreference.value = true
        clearAIError() // Clear any previous errors

        // Trigger download if model not available
        if (!modelStatus.value?.available) {
          await triggerDownload()
        }
      }
    }
    catch (error) {
      toast.remove(TOAST_IDS.ERROR)
      toast.add({
        id: TOAST_IDS.ERROR,
        title: "Error",
        description: error instanceof Error ? error.message : "Failed to toggle AI settings",
        color: "error",
        duration: 5000,
      })
    }
  }

  // Set AI error
  function setAIError(error: string | Error) {
    const errorMessage = error instanceof Error ? error.message : error
    const errorDetails = error instanceof Error && error.stack ? error.stack : errorMessage

    aiError.value = {
      message: errorMessage,
      details: errorDetails,
      timestamp: new Date(),
    }
  }

  // Clear AI error
  function clearAIError() {
    aiError.value = null
  }

  return {
    modelStatus: readonly(modelStatus),
    isDownloading: readonly(isDownloading),
    aiPreference: readonly(aiPreference),
    aiEnabled,
    aiStatus: readonly(aiStatus),
    aiError: readonly(aiError),
    toggleAI,
    setAIError,
    clearAIError,
  }
}