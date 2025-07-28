import { commands } from "~/utils/bindings"
import type { ModelStatus } from "~/utils/bindings"
import { listen } from "@tauri-apps/api/event"
import { useModelState } from "./modelProvider"

export function useAIStatus() {
  const modelState = useModelState()
  const aiSettingsStore = useAISettingsStore()
  const modelStatus = ref<ModelStatus | null>(null)
  const isChecking = ref(false)

  // Use shared download state
  const isDownloading = computed(() => modelState.isDownloading)

  // Check model status
  async function checkStatus() {
    isChecking.value = true
    try {
      const result = await commands.checkModelStatus()
      if (result.status === "ok") {
        modelStatus.value = result.data
      }
    }
    catch (error) {
      console.error("Failed to check model status:", error)
    }
    finally {
      isChecking.value = false
    }
  }

  // Listen for model download completion
  onMounted(async () => {
    // Initial check
    await checkStatus()

    // Listen for model status changes
    const unlisten = await listen("model-status-changed", async () => {
      await checkStatus()
    })

    onScopeDispose(() => {
      unlisten()
    })
  })

  // Manual download trigger
  async function triggerDownload() {
    // Ensure model status is available - backend should always provide this
    if (!modelStatus.value) {
      throw new Error("Model status not available - backend may not be responding")
    }

    // Check preference first
    if (!aiSettingsStore.aiEnabled) {
      // User previously disabled AI, update preference to enable and trigger
      aiSettingsStore.aiEnabled = true
    }

    // Use reactive state instead of events - backend guarantees these values exist
    modelState.requestDownload({
      name: modelStatus.value.modelName,
      size: modelStatus.value.modelSize,
    })
  }

  return {
    modelStatus: readonly(modelStatus),
    isChecking: readonly(isChecking),
    isDownloading: readonly(isDownloading),
    checkStatus,
    triggerDownload,
  }
}