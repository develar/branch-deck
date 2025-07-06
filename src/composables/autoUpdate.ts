import { relaunch } from "@tauri-apps/plugin-process"
import { listen } from "@tauri-apps/api/event"
import { onMounted, onUnmounted } from "vue"
import { useTimeoutFn } from "@vueuse/core"
import { check, Update } from "@tauri-apps/plugin-updater"
import * as log from "@tauri-apps/plugin-log"
import { getName, getVersion } from "@tauri-apps/api/app"

export function useAutoUpdate() {
  const toast = useToast()
  let unlistenCheckForUpdates: (() => void) | null = null

  async function checkForUpdates(showNoUpdateToast: boolean = false) {
    try {
      const update = await check()
      if (update != null) {
        showUpdateAvailableToast(update)
      }
      else if (showNoUpdateToast) {
        toast.add({
          title: "You’re up to date!",
          description: `${await getName()} ${await getVersion()} is currently the newest version available.`,
          icon: "i-lucide-check",
          color: "success",
        })
      }
    }
    catch (error) {
      await log.error(`Failed to check for updates: ${error}`)
      if (showNoUpdateToast) {
        showUpdateErrorToast("Could not check for updates. Please try again later.")
      }
    }
  }

  function showUpdateAvailableToast(update: Update) {
    toast.add({
      title: "Update available",
      description: `Version ${update.version} is available. Click to download and install.`,
      icon: "i-lucide-download",
      color: "info",
      duration: 10_000,
      actions: [
        {
          label: "Update now",
          onClick: () => downloadAndInstallUpdate(update),
        },
        {
          label: "Later",
          onClick: () => {
          },
        },
      ],
    })
  }

  function showUpdateErrorToast(message: string) {
    toast.add({
      title: "Update check failed",
      description: message,
      icon: "i-lucide-triangle-alert",
      color: "error",
    })
  }

  async function downloadAndInstallUpdate(update: Update) {
    try {
      // show download progress toast
      const downloadToast = toast.add({
        title: "Downloading update...",
        description: `Downloading version ${update.version}`,
        icon: "i-lucide-download",
        color: "info",
        duration: 10_000_000,
        progress: false,
      })

      // download and install the update
      await update.downloadAndInstall()

      // remove download toast
      toast.remove(downloadToast.id)
      await relaunch()
    }
    catch (error) {
      await log.error(`Failed to download and install update: ${error}`)
      toast.add({
        title: "Update failed",
        description: `Failed to download or install the update (${error instanceof Error ? error.message : error}).`,
        icon: "i-lucide-triangle-alert",
        color: "error",
      })
    }
  }

  // check for updates on app startup (silently)
  function checkForUpdatesOnStartup() {
    // wait a bit after app startup to avoid interfering with initial load
    useTimeoutFn(() => {
      checkForUpdates(false).catch(error => log.error(`${error}`))
    }, 3_000)
  }

  // Set up event listeners for menu-triggered update checks
  onMounted(async () => {
    // Listen for update available event from menu
    unlistenCheckForUpdates = await listen("check_for_updates", () => {
      const checkForUpdatesPromise = checkForUpdates(true)

      const progressToastId = toast.add({
        title: "Checking for updates…",
        color: "info",
        progress: false,
        close: false,
      }).id

      checkForUpdatesPromise
        .catch((error) => {
          log.error(`${error}`)
        })
        .finally(() => {
          toast.remove(progressToastId)
        })
    })
  })

  // clean up listeners
  onUnmounted(() => {
    if (unlistenCheckForUpdates) {
      unlistenCheckForUpdates()
      unlistenCheckForUpdates = null
    }
  })

  return { checkForUpdates, checkForUpdatesOnStartup }
}
