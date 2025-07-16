import { listen } from "@tauri-apps/api/event"
import { onMounted, onUnmounted } from "vue"
import { useTimeoutFn } from "@vueuse/core"
import * as log from "@tauri-apps/plugin-log"
import { getName } from "@tauri-apps/api/app"
import { commands } from "~/utils/bindings"
import type { UpdateInfo } from "~/utils/bindings"

export function useAutoUpdate() {
  const toast = useToast()
  let unlistenCheckForUpdates: (() => void) | null = null

  async function checkForUpdates(showNoUpdateToast: boolean = false) {
    try {
      const updateInfo = await commands.checkForUpdates()
      if (updateInfo.status === "ok") {
        if (updateInfo.data.is_update_available) {
          showUpdateAvailableToast(updateInfo.data)
        }
        else if (showNoUpdateToast) {
          toast.add({
            title: "You're up to date!",
            description: `${await getName()} ${updateInfo.data.current_version} is currently the newest version available.`,
            icon: "i-lucide-check",
            color: "success",
          })
        }
      }
      else if (showNoUpdateToast) {
        showUpdateErrorToast()
      }
    }
    catch (error) {
      await log.error(`Failed to check for updates: ${error}`)
      if (showNoUpdateToast) {
        showUpdateErrorToast()
      }
    }
  }

  function showUpdateAvailableToast(updateInfo: UpdateInfo) {
    // By the time we show this toast, the update is already downloaded (from check command)
    toast.add({
      title: "Update available",
      description: `Version ${updateInfo.available_version} is ready to install.`,
      icon: "i-lucide-download",
      color: "info",
      duration: 0,
      actions: [
        {
          label: "Install now and restart",
          onClick: () => installUpdateNow(),
        },
        {
          label: "Later",
          onClick: () => {
          },
        },
      ],
    })
  }

  function showUpdateErrorToast(message: string = "Could not check for updates. Please try again later.") {
    toast.add({
      title: "Update check failed",
      description: message,
      icon: "i-lucide-triangle-alert",
      color: "error",
    })
  }

  async function installUpdateNow() {
    try {
      await commands.installUpdate()
    }
    catch (error) {
      await log.error(`Failed to install update: ${error}`)
      toast.add({
        title: "Update failed",
        description: `Failed to install the update (${error instanceof Error ? error.message : error}).`,
        icon: "i-lucide-triangle-alert",
        color: "error",
      })
    }
  }

  // check for updates on app startup (silently)
  function checkForUpdatesOnStartup() {
    // Skip update checks in development mode
    if (import.meta.env.DEV) {
      return
    }

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
        duration: 0,
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
