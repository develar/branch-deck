// Model information for download UI
export interface ModelInfo {
  name: string
  size: string
}

// Injection key
export const ModelStateKey = Symbol("model-state")

// Model state interface
export interface ModelState {
  // State
  isDownloading: Ref<boolean>
  progress: Ref<number>
  currentFile: Ref<string>
  needsDownload: Ref<boolean>
  downloadRequested: Ref<boolean>
  modelInfo: Readonly<Ref<ModelInfo | null>>

  // Actions
  requestDownload: (info: ModelInfo) => void
  clearDownloadRequest: () => void
}

// Create model state
export function createModelState(): ModelState {
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
}

// Provide model state
export function provideModelState() {
  const state = createModelState()
  provide(ModelStateKey, state)
  return state
}

// Use model state
export function useModelState(): ModelState {
  const state = inject<ModelState>(ModelStateKey)
  if (!state) {
    throw new Error("Model state not provided. Make sure to call provideModelState() at the app level.")
  }
  return state
}