import { useEffect } from "react"
import { getName } from "@tauri-apps/api/app"
import { toast } from "@/components/ui/toast"
import { useTauriListen } from "@/hooks/useTauriListen"
import { commands } from "@/lib/bindings"
import type { UpdateInfo } from "@/lib/bindings"
import { getErrorDetails, logError } from "@/lib/errors"

const UPDATE_AVAILABLE_TOAST_ID = "update-available"
// wait a bit after app startup to avoid interfering with the initial load
const STARTUP_CHECK_DELAY_MS = 3_000

// the startup check runs once per app session (StrictMode mounts effects twice)
let isStartupCheckScheduled = false

/**
 * Checks for updates and reports the result with toasts.
 * An available update is already downloaded by the check command, so the toast offers to install it right away.
 * Without `showNoUpdateToast` (silent startup check), only an available update is reported.
 */
export async function checkForUpdates(showNoUpdateToast = false): Promise<void> {
  try {
    const result = await commands.checkForUpdates()
    if (result.status === "ok") {
      if (result.data.is_update_available) {
        showUpdateAvailableToast(result.data)
      }
      else if (showNoUpdateToast) {
        toast.add({
          type: "success",
          title: "You're up to date",
          description: `${await getName()} ${result.data.current_version} is currently the newest version available.`,
        })
      }
    }
    else {
      logError("Failed to check for updates", result.error)
      if (showNoUpdateToast) {
        showUpdateErrorToast()
      }
    }
  }
  catch (error) {
    logError("Failed to check for updates", error)
    if (showNoUpdateToast) {
      showUpdateErrorToast()
    }
  }
}

function showUpdateAvailableToast(updateInfo: UpdateInfo): void {
  // a repeated check (menu) replaces the previous toast instead of stacking another one
  toast.close(UPDATE_AVAILABLE_TOAST_ID)
  toast.add({
    id: UPDATE_AVAILABLE_TOAST_ID,
    type: "info",
    title: "Update available",
    description: `Version ${updateInfo.available_version} is ready to install.`,
    // stays until the user acts; the close button means "later"
    timeout: 0,
    actionProps: {
      children: "Install now and restart",
      onClick: () => {
        toast.close(UPDATE_AVAILABLE_TOAST_ID)
        void installUpdateNow()
      },
    },
  })
}

function showUpdateErrorToast(message = "Could not check for updates. Please try again later."): void {
  toast.add({
    type: "error",
    title: "Update check failed",
    description: message,
  })
}

async function installUpdateNow(): Promise<void> {
  let failure: unknown = null
  try {
    // the app restarts on success
    const result = await commands.installUpdate()
    if (result.status === "error") {
      failure = result.error
    }
  }
  catch (error) {
    failure = error
  }

  if (failure !== null) {
    logError("Failed to install update", failure)
    toast.add({
      type: "error",
      title: "Update failed",
      description: `Failed to install the update (${getErrorDetails(failure)}).`,
    })
  }
}

/** Schedules a silent update check shortly after startup; runs once per app session and never in development. */
function checkForUpdatesOnStartup(): void {
  if (import.meta.env.DEV || isStartupCheckScheduled) {
    return
  }
  isStartupCheckScheduled = true

  setTimeout(() => {
    void checkForUpdates(false)
  }, STARTUP_CHECK_DELAY_MS)
}

/** Main window: silent update check on startup and the "Check for Updates…" menu item. */
export function useAutoUpdate(): void {
  useEffect(() => {
    checkForUpdatesOnStartup()
  }, [])

  useTauriListen("check_for_updates", () => {
    const progressToastId = toast.add({
      type: "loading",
      title: "Checking for updates…",
      timeout: 0,
    })
    void checkForUpdates(true).finally(() => {
      toast.close(progressToastId)
    })
  })
}
