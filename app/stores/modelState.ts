import { defineStore } from "pinia"

// Model information for download UI
export interface ModelInfo {
  name: string
  size: string
}

export const useModelStore = defineStore("model", () => {
  // Download progress state
  const isDownloading = ref(false)
  const progress = ref(0)
  const currentFile = ref("")

  // Download request state - replaces event-driven approach
  const needsDownload = ref(false)
  const downloadRequested = ref(false)
  const modelInfo = ref<ModelInfo | null>(null)

  // Actions
  function requestDownload(info: ModelInfo) {
    // Idempotency protection - don't trigger duplicate requests
    if (needsDownload.value || isDownloading.value) {
      return // Already requested or downloading, ignore duplicate calls
    }

    needsDownload.value = true
    modelInfo.value = info
  }

  function clearDownloadRequest() {
    needsDownload.value = false
    downloadRequested.value = false
    modelInfo.value = null
  }

  return {
    // State
    isDownloading,
    progress,
    currentFile,
    needsDownload,
    downloadRequested,
    modelInfo: readonly(modelInfo),

    // Actions
    requestDownload,
    clearDownloadRequest,
  }
})