import { Channel } from "@tauri-apps/api/core"
import { appStore } from "~/utils/app-store"
import { commands } from "~/utils/bindings"
import type { DownloadProgress } from "~/utils/bindings"
import { useModelStore } from "~/stores/modelState"

export function useModelDownload() {
  const toast = useToast()
  const modelStore = useModelStore()

  // Watch reactive state for download requests instead of using events
  watchEffect(async () => {
    // Skip if no download needed or already downloading
    if (!modelStore.needsDownload || modelStore.isDownloading) {
      return
    }

    const modelInfo = modelStore.modelInfo
    if (!modelInfo) {
      return
    }

    // Check if we already have a preference
    const modelSettings = await appStore.getModelSettings()

    if (modelSettings.aiEnabled === false) {
      // User has disabled AI, don't ask again
      modelStore.clearDownloadRequest()
      return
    }

    if (modelSettings.aiEnabled === true) {
      // User has enabled AI, start download automatically
      modelStore.downloadRequested = true
      startDownload(modelInfo.name)
      return
    }

    // No preference set, show toast with action buttons (only once)
    if (!modelStore.downloadRequested) {
      modelStore.downloadRequested = true

      toast.add({
        title: "AI Model Required",
        description: `Branch name suggestions require downloading ${modelInfo.name} (${modelInfo.size}). This is a one-time download.`,
        color: "primary",
        actions: [
          {
            label: "Don't Download",
            color: "neutral",
            variant: "outline",
            onClick: async () => {
              // Store preference and clear request
              await appStore.updateModelSetting("aiEnabled", false)
              modelStore.clearDownloadRequest()
            },
          },
          {
            label: `Download Model (${modelInfo.size})`,
            color: "primary",
            variant: "solid",
            onClick: async () => {
              // Store preference and start download
              await appStore.updateModelSetting("aiEnabled", true)
              startDownload(modelInfo.name)
            },
          },
        ],
      })
    }
  })

  async function startDownload(_modelName: string) {
    // Clear download request since we're starting the download
    modelStore.clearDownloadRequest()

    // Update shared state
    modelStore.isDownloading = true
    modelStore.progress = 0
    modelStore.currentFile = ""

    // Create a progress toast with a stable ID that we'll update
    const progressToastId = "model-download-progress"
    toast.add({
      id: progressToastId,
      title: "Downloading AI Model",
      description: "Preparing download...",
      duration: 0, // Keep toast open until we close it
    })

    // Throttle progress updates to max 1 per second
    let lastUpdateTime = 0
    const UPDATE_THROTTLE_MS = 1000 // ~1 update per second

    try {
      // Create channel for progress updates
      const channel = new Channel<DownloadProgress>()
      channel.onmessage = (event: DownloadProgress) => {
        if (event.type === "Progress") {
          const now = Date.now()

          // Throttle updates to prevent UI jank
          if (now - lastUpdateTime < UPDATE_THROTTLE_MS) {
            return
          }
          lastUpdateTime = now

          const { fileName, downloaded, total, bytesPerSecond, secondsRemaining } = event.data
          const percentage = Math.round((downloaded / total) * 100)

          // Update shared state
          modelStore.progress = percentage
          modelStore.currentFile = fileName

          // Format downloaded/total size
          const downloadedMB = (downloaded / (1024 * 1024)).toFixed(1)
          const totalMB = (total / (1024 * 1024)).toFixed(1)

          let description = `Downloading ${fileName}: ${downloadedMB}/${totalMB} MB (${percentage}%)`

          // Add speed if available
          if (bytesPerSecond) {
            const mbPerSecond = (bytesPerSecond / (1024 * 1024)).toFixed(1)
            description += ` • ${mbPerSecond} MB/s`
          }

          // Add time remaining if available
          if (secondsRemaining !== undefined && secondsRemaining !== null) {
            if (secondsRemaining === 0) {
              description += " • Finishing..."
            }
            else if (secondsRemaining < 60) {
              description += ` • ${secondsRemaining}s remaining`
            }
            else {
              const minutes = Math.floor(secondsRemaining / 60)
              const seconds = secondsRemaining % 60
              description += ` • ${minutes}m ${seconds}s remaining`
            }
          }

          // Update the existing toast
          toast.update(progressToastId, {
            description,
          })
        }
        else if (event.type === "Completed") {
          // Remove progress toast
          toast.remove(progressToastId)

          // Show success toast
          toast.add({
            title: "Model Downloaded",
            description: "AI model downloaded successfully. Branch name suggestions are now available.",
            color: "success",
          })

          // Model download completed - backend status will reflect this automatically

          // Update shared state
          modelStore.isDownloading = false
          modelStore.progress = 100

          // Emit event so UI can update (keeping model-status-changed for now)
          import("@tauri-apps/api/event").then(({ emit }) => {
            emit("model-status-changed", {})
          })
        }
        else if (event.type === "Error") {
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
          modelStore.isDownloading = false
        }
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
      modelStore.isDownloading = false
    }
  }

  return {
    // Expose for manual triggering if needed
    startDownload,
  }
}