import { Channel } from "@tauri-apps/api/core"
import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import type { DownloadProgress, ModelStatus } from "@/lib/bindings"
import { getErrorDetails, logError } from "@/lib/errors"
import { setAiMode } from "@/stores/settings/aiSettings"

/** Model information for the download UI, reported by the backend when suggestions need the model */
export interface ModelInfo {
  /** Display name of the AI model */
  name: string
  /** Human-readable size of the model (e.g., "2.5 GB") */
  size: string
}

export interface AIError {
  /** User-friendly error message */
  message: string
  /** Detailed error information (e.g., stack trace) */
  details: string
  /** When the error occurred (ms since epoch) */
  timestamp: number
}

export interface ModelState {
  /** Set by `requestDownload` */
  modelInfo: ModelInfo | null
  /** Result of the last `checkModelStatus` call (model name, size and Hugging Face page, file presence) */
  modelStatus: ModelStatus | null
  isDownloading: boolean
  /** Last download event; `Progress` events are throttled to one store update per second */
  lastProgressEvent: DownloadProgress | null
  /** Last suggestion generation failure; shown by the AI status indicator */
  aiError: AIError | null
}

/** Id of the live download progress toast */
export const MODEL_DOWNLOAD_TOAST_ID = "model-download-progress"
/** `data.kind` of the live download progress toast; `Toaster` renders `ModelDownloadProgress` as its body */
export const MODEL_DOWNLOAD_TOAST_KIND = "model-download"

const PROGRESS_THROTTLE_MS = 1000

export const modelStore = createStore<ModelState>(() => ({
  modelInfo: null,
  modelStatus: null,
  isDownloading: false,
  lastProgressEvent: null,
  aiError: null,
}))

export function useModelStore<T>(selector: (state: ModelState) => T): T {
  return useStore(modelStore, selector)
}

/**
 * Starts the model download unless one is already running (idempotent).
 * Called when the backend reports that suggestions need the model.
 */
export function requestDownload(info: ModelInfo): void {
  if (modelStore.getState().isDownloading) {
    return
  }
  modelStore.setState({ modelInfo: info })
  void downloadModel()
}

/** Downloads the model with a live progress toast; resolves when the download ends. Ignored while a download runs. */
export async function downloadModel(): Promise<void> {
  if (modelStore.getState().isDownloading) {
    return
  }

  modelStore.setState({ isDownloading: true, lastProgressEvent: null })

  // the toast body subscribes to the store itself, so the toast is not updated per event
  toast.close(MODEL_DOWNLOAD_TOAST_ID)
  toast.add({
    id: MODEL_DOWNLOAD_TOAST_ID,
    title: "Downloading AI Model",
    // keep open until the download ends
    timeout: 0,
    data: { kind: MODEL_DOWNLOAD_TOAST_KIND },
    actionProps: {
      children: "Pause",
      onClick: () => void cancelDownload(),
    },
  })

  let lastProgressUpdate = 0
  // the download may end without a terminal event (e.g., the command fails before streaming)
  let endedViaEvent = false

  const channel = new Channel<DownloadProgress>()
  channel.onmessage = (event) => {
    switch (event.type) {
      case "Started":
      case "FileStarted":
      case "FileCompleted": {
        modelStore.setState({ lastProgressEvent: event })
        break
      }

      case "Progress": {
        // throttle to prevent UI jank
        const now = Date.now()
        if (now - lastProgressUpdate < PROGRESS_THROTTLE_MS) {
          return
        }
        lastProgressUpdate = now
        modelStore.setState({ lastProgressEvent: event })
        break
      }

      case "Completed": {
        endedViaEvent = true
        toast.close(MODEL_DOWNLOAD_TOAST_ID)
        toast.add({
          type: "success",
          title: "Model Downloaded",
          description: "AI model downloaded successfully. Branch name suggestions are now available.",
        })
        modelStore.setState({ isDownloading: false, lastProgressEvent: event })
        // keep the completion visible for a moment
        setTimeout(() => {
          if (!modelStore.getState().isDownloading) {
            modelStore.setState({ lastProgressEvent: null })
          }
        }, 1000)
        break
      }

      case "Cancelled": {
        endedViaEvent = true
        toast.close(MODEL_DOWNLOAD_TOAST_ID)
        toast.add({
          title: "Download Paused",
          description: "Download paused. Enable AI to resume.",
        })
        modelStore.setState({ isDownloading: false, lastProgressEvent: null })
        // the model isn't available
        setAiMode("disabled")
        break
      }

      case "Error": {
        endedViaEvent = true
        failDownload(event.data.message)
        break
      }
    }
  }

  try {
    const result = await commands.downloadModel(channel)
    if (result.status === "error" && !endedViaEvent) {
      endedViaEvent = true
      logError("Model download failed", result.error)
      failDownload(result.error.message)
    }
  }
  catch (error) {
    if (!endedViaEvent) {
      endedViaEvent = true
      logError("Model download failed", error)
      failDownload(error instanceof Error ? error.message : "An unexpected error occurred")
    }
  }
  finally {
    if (!endedViaEvent && modelStore.getState().isDownloading) {
      console.warn("[model] Download ended without a completion event, resetting state")
      toast.close(MODEL_DOWNLOAD_TOAST_ID)
      modelStore.setState({ isDownloading: false })
    }
  }
}

function failDownload(message: string): void {
  toast.close(MODEL_DOWNLOAD_TOAST_ID)
  toast.add({
    type: "error",
    title: "Download Failed",
    description: message,
  })
  modelStore.setState({ isDownloading: false, lastProgressEvent: null })
}

/** Cancels the running download; the backend answers with a `Cancelled` event that resets the state. */
export async function cancelDownload(): Promise<void> {
  if (!modelStore.getState().isDownloading) {
    return
  }

  try {
    const result = await commands.cancelModelDownload()
    if (result.status === "error") {
      logError("Failed to cancel model download", result.error)
    }
  }
  catch (error) {
    logError("Failed to cancel model download", error)
  }
}

/** Queries the model description and whether the model files are present */
export async function checkModelStatus(): Promise<ModelStatus | null> {
  let result: Awaited<ReturnType<typeof commands.checkModelStatus>>
  try {
    result = await commands.checkModelStatus()
  }
  catch (error) {
    logError("Failed to check model status", error)
    return null
  }
  if (result.status === "error") {
    logError("Failed to check model status", result.error)
    return null
  }
  modelStore.setState({ modelStatus: result.data })
  return result.data
}

let isModelStatusRequested = false

/** Loads the model status once (e.g., for the model link in the AI help popover); a failed request is retried on the next call */
export function ensureModelStatus(): void {
  if (isModelStatusRequested || modelStore.getState().modelStatus !== null) {
    return
  }
  isModelStatusRequested = true
  void checkModelStatus().then((status) => {
    if (status === null) {
      isModelStatusRequested = false
    }
  })
}

export function setAIError(error: unknown): void {
  const message = error instanceof Error ? error.message : getErrorDetails(error)
  const details = error instanceof Error && error.stack ? error.stack : message
  modelStore.setState({ aiError: { message, details, timestamp: Date.now() } })
}

export function clearAIError(): void {
  if (modelStore.getState().aiError !== null) {
    modelStore.setState({ aiError: null })
  }
}
