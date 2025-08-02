import { Channel } from "@tauri-apps/api/core"
import { h } from "vue"
import type { DownloadProgress } from "~/utils/bindings"
import { commands } from "~/utils/bindings"
import ModelDownloadProgress from "#layers/ai/components/ModelDownloadProgress.vue"
import { useThrottleFn } from "@vueuse/core"

/**
 * Model information for download UI
 */
export interface ModelInfo {
  /** Display name of the AI model */
  name: string
  /** Human-readable size of the model (e.g., "1.2 GB") */
  size: string
}

/** Injection key for providing model state across the application */
export const ModelStateKey = Symbol("model-state")

interface DownloadStateRefs {
  isDownloading: Ref<boolean>
  progress: Ref<number>
  currentFile: Ref<string>
  lastProgressEvent: Ref<DownloadProgress | null>
}

/**
 * Handles download progress events and updates UI accordingly
 */
function handleDownloadProgress(
  event: DownloadProgress,
  state: DownloadStateRefs,
  toast: ReturnType<typeof useToast>,
  progressToastId: string,
  throttleState: { lastUpdateTime: number },
) {
  // Store the last progress event
  state.lastProgressEvent.value = event

  const UPDATE_THROTTLE_MS = 1000

  switch (event.type) {
    case "Started": {
      toast.update(progressToastId, {
        description: h(ModelDownloadProgress, {
          downloadProgress: event,
          showCancelButton: false,
        }),
      })
      break
    }

    case "FileStarted": {
      toast.update(progressToastId, {
        description: h(ModelDownloadProgress, {
          downloadProgress: event,
          showCancelButton: false,
        }),
      })
      break
    }

    case "Progress": {
      const now = Date.now()
      // Throttle updates to prevent UI jank
      if (now - throttleState.lastUpdateTime < UPDATE_THROTTLE_MS) {
        return
      }
      throttleState.lastUpdateTime = now

      const { downloaded, total } = event.data
      const percentage = Math.round((downloaded / total) * 100)

      // Update shared state
      state.progress.value = percentage
      state.currentFile.value = event.data.fileName

      // Update the existing toast with the component
      toast.update(progressToastId, {
        description: h(ModelDownloadProgress, {
          downloadProgress: event,
          showCancelButton: false,
        }),
      })
      break
    }

    case "Completed": {
      // Remove progress toast
      toast.remove(progressToastId)

      // Show success toast
      toast.add({
        title: "Model Downloaded",
        description: "AI model downloaded successfully. Branch name suggestions are now available.",
        color: "success",
      })

      // Update shared state
      state.isDownloading.value = false
      state.progress.value = 100

      // Clear progress event after a short delay to show completion
      setTimeout(() => {
        state.lastProgressEvent.value = null
      }, 1000)
      break
    }

    case "Cancelled": {
      // Remove progress toast
      toast.remove(progressToastId)

      // Show cancellation toast
      toast.add({
        title: "Download Paused",
        description: "Download paused. Enable AI to resume.",
      })

      // Update shared state
      state.isDownloading.value = false
      state.lastProgressEvent.value = null

      // Disable AI since model isn't available
      // We need to load the store asynchronously but can't await in the event handler
      useAISettingsStore().then((store) => {
        store.aiMode = "disabled"
      })
      break
    }

    case "Error": {
      const { message } = event.data

      // Remove progress toast
      toast.remove(progressToastId)

      // Show error toast
      toast.add({
        title: "Download Failed",
        description: message,
        color: "error",
      })

      // Update shared state
      state.isDownloading.value = false
      state.lastProgressEvent.value = null
      break
    }
  }
}

/**
 * Performs the actual model download with progress tracking
 */
async function performModelDownload(
  state: DownloadStateRefs,
  cancelDownload: () => Promise<void>,
) {
  const toast = useToast()
  const progressToastId = "model-download-progress"
  const throttleState = { lastUpdateTime: 0 }

  // Track if download completed successfully through events
  let downloadCompletedViaEvent = false

  try {
    // Create toast with initial state BEFORE setting up channel
    console.log("[ModelProvider] Creating download progress toast")
    toast.add({
      id: progressToastId,
      title: "Downloading AI Model",
      description: h(ModelDownloadProgress, {
        downloadProgress: null, // Will show "Initializing download..."
        showCancelButton: false,
      }),
      duration: 0, // Keep toast open until we close it
      actions: [
        {
          label: "Pause",
          color: "neutral",
          variant: "ghost",
          onClick: async () => {
            await cancelDownload()
          },
        },
      ],
    })

    // Create channel for progress updates
    const channel = new Channel<DownloadProgress>()
    channel.onmessage = (event: DownloadProgress) => {
      console.log("[ModelProvider] Download event received:", event.type)
      // Track if download completed via event
      if (event.type === "Completed" || event.type === "Cancelled" || event.type === "Error") {
        downloadCompletedViaEvent = true
      }
      handleDownloadProgress(event, state, toast, progressToastId, throttleState)
    }

    // Start download with progress channel
    await commands.downloadModel(channel)
  }
  catch (error) {
    // Remove progress toast
    toast.remove(progressToastId)

    // Show error toast
    toast.add({
      title: "Download Failed",
      description: error instanceof Error ? error.message : "An unexpected error occurred",
      color: "error",
    })

    // Update shared state
    state.isDownloading.value = false
  }
  finally {
    // Ensure isDownloading is reset if not already done via events
    // This handles edge cases where the download fails without sending an event
    if (!downloadCompletedViaEvent && state.isDownloading.value) {
      console.warn("[ModelProvider] Download ended without completion event, resetting state")
      state.isDownloading.value = false
    }
  }
}

/**
 * Model state interface for managing AI model downloads
 */
export interface ModelState {
  /** Whether a model download is currently in progress */
  isDownloading: Ref<boolean>
  /** Information about the model being downloaded (readonly) */
  modelInfo: Readonly<Ref<ModelInfo | null>>
  /** Last download progress event from backend (readonly) */
  lastProgressEvent: Readonly<Ref<DownloadProgress | null>>

  /**
   * Request to download a model. This is idempotent - duplicate calls are ignored.
   * @param info - Model information including name and size
   */
  requestDownload: (info: ModelInfo) => void

  /**
   * Cancel an ongoing model download
   */
  cancelDownload: () => Promise<void>
}

/**
 * Creates a new model state instance with download management capabilities.
 * Handles the complete lifecycle of model downloads including progress tracking,
 * user preferences, and error handling.
 *
 * @returns ModelState object with reactive state and actions
 */
export function createModelState(): ModelState {
  // Download progress state
  const isDownloading = ref(false)
  const progress = ref(0)
  const currentFile = ref("")
  const lastProgressEvent = ref<DownloadProgress | null>(null)

  // Download request state - replaces event-driven approach
  const needsDownload = ref(false)
  const downloadRequested = ref(false)
  const modelInfo = shallowRef<ModelInfo | null>(null)

  /**
   * Clears all download request state.
   * Resets download flags and model information to initial values.
   * @internal
   */
  function clearDownloadRequest() {
    needsDownload.value = false
    downloadRequested.value = false
    modelInfo.value = null
  }

  /**
   * Requests to download a model. This operation is idempotent - duplicate
   * calls are safely ignored if a download is already in progress or requested.
   *
   * @param info - Model information containing name and size for display
   * @example
   * requestDownload({ name: "llama-3.2", size: "1.2 GB" })
   */
  function requestDownload(info: ModelInfo) {
    // Idempotency protection - don't trigger duplicate requests
    if (needsDownload.value || isDownloading.value) {
      console.log("[ModelProvider] requestDownload ignored - already requested or downloading", { needsDownload: needsDownload.value, isDownloading: isDownloading.value })
      return // Already requested or downloading, ignore duplicate calls
    }

    console.log("[ModelProvider] requestDownload called", info)
    needsDownload.value = true
    modelInfo.value = info
  }

  /**
   * Cancel an ongoing download
   */
  async function cancelDownload() {
    if (!isDownloading.value) {
      return
    }

    try {
      const result = await commands.cancelModelDownload()
      if (result.status === "ok") {
        // The backend will send a Cancelled event through the progress channel
        // which will update our state
      }
    }
    catch (error) {
      console.error("Failed to cancel download:", error)
    }
  }

  // Handle download request
  // Throttle the download request processing
  const processDownloadRequestThrottled = useThrottleFn(
    () => {
      console.log("[ModelProvider] Processing download request...")

      // If requestDownload was called, the calling code has already
      // determined that a download should happen. Just start it.
      downloadRequested.value = true
      if (isDownloading.value) {
        return
      }

      // Initialize state
      isDownloading.value = true
      progress.value = 0
      currentFile.value = ""
      lastProgressEvent.value = null
      clearDownloadRequest()

      // Delegate to the extracted function
      // Don't await - let it run in background
      // noinspection JSIgnoredPromiseFromCall
      performModelDownload(
        { isDownloading, progress, currentFile, lastProgressEvent },
        cancelDownload,
      )
    },
    300,
    true, // trailing: execute after the throttle period
    true, // leading: execute immediately on first call
  )

  // Synchronous watcher that triggers throttled async processing
  watch(
    [() => needsDownload.value, () => modelInfo.value],
    ([needsDownloadValue, modelInfoValue]) => {
      // Skip if no download needed
      if (!needsDownloadValue) {
        console.log("[ModelProvider] Watcher skipped - no download needed", { needsDownloadValue })
        return
      }

      if (!modelInfoValue) {
        console.log("[ModelProvider] Watcher skipped - no model info")
        return
      }

      // Skip if already downloading
      if (isDownloading.value) {
        console.log("[ModelProvider] Watcher skipped - already downloading")
        return
      }

      console.log("[ModelProvider] Watcher triggered, processing download request...")
      console.log("[ModelProvider] Current state:", { isDownloading: isDownloading.value, needsDownload: needsDownload.value })
      // Trigger throttled async processing
      processDownloadRequestThrottled()
    },
  )

  return {
    // State
    isDownloading,
    modelInfo: readonly(modelInfo),
    lastProgressEvent: readonly(lastProgressEvent),

    // Actions
    requestDownload,
    cancelDownload,
  }
}

/**
 * Provides model state to the component tree using Vue's provide/inject API.
 * Should be called at the application root level.
 *
 * @returns The created model state instance
 */
export function provideModelState() {
  const state = createModelState()
  provide(ModelStateKey, state)
  return state
}

/**
 * Retrieves the model state from the component tree.
 * Must be called within a component that has access to the provided state.
 *
 * @returns The model state instance
 * @throws Error if model state has not been provided
 */
export function useModelState(): ModelState {
  const state = inject<ModelState>(ModelStateKey)
  if (!state) {
    throw new Error("Model state not provided. Make sure to call provideModelState() at the app level.")
  }
  return state
}
