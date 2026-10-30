import { useEffect } from "react"
import { getName } from "@tauri-apps/api/app"
import { toast } from "@/components/ui/toast"
import { useAppCommand } from "@/hooks/useAppCommand"
import { commands } from "@/lib/bindings"
import type { UpdateInfo } from "@/lib/bindings"
import { logError, notifyError } from "@/lib/errors"
import { refreshSetup } from "@/stores/setup"

const UPDATE_AVAILABLE_TOAST_ID = "update-available"
// wait a bit after app startup to avoid interfering with the initial load
const STARTUP_CHECK_DELAY_MS = 3_000
const UPDATE_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000

/**
 * Checks for updates and reports the result with toasts.
 * An available update is already downloaded by the check command, so the toast offers to install it right away.
 * Without `showNoUpdateToast` (silent checks), an available update goes to the Setup dot; failures refresh Setup and are logged.
 */
export async function checkForUpdates(showNoUpdateToast = false): Promise<void> {
  let failure: unknown
  try {
    const result = await commands.checkForUpdates()
    if (result.status === "ok") {
      if (result.data.is_update_available) {
        // asked: a toast; at startup the Setup dot keeps it, as nobody asked (spec/repository/setup-attention.spec.md)
        if (showNoUpdateToast) {
          showUpdateAvailableToast(result.data)
        }
      } else if (showNoUpdateToast) {
        toast.add({
          type: "success",
          title: `${await getName()} is up to date`,
          description: `${result.data.current_version} is the newest version.`,
        })
      }
      void refreshSetup()
      return
    }
    failure = result.error
  } catch (error) {
    failure = error
  }
  if (showNoUpdateToast) {
    notifyError("Cannot check for updates", failure)
  } else {
    logError("Cannot check for updates", failure)
  }
  void refreshSetup()
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
      children: "Install and Restart",
      onClick: () => {
        toast.close(UPDATE_AVAILABLE_TOAST_ID)
        void installUpdateNow()
      },
    },
  })
}

/** Installs the downloaded update; the app restarts on success, a failure is a toast */
export async function installUpdateNow(): Promise<void> {
  let failure: unknown = null
  try {
    // the app restarts on success
    const result = await commands.installUpdate()
    if (result.status === "error") {
      failure = result.error
    }
  } catch (error) {
    failure = error
  }

  if (failure !== null) {
    notifyError("Cannot install the update", failure)
  }
}

/** Schedules a silent update check shortly after startup, then every six hours; never in development. */
function checkForUpdatesOnStartup(): (() => void) | undefined {
  if (import.meta.env.DEV) {
    return
  }

  let every: ReturnType<typeof setInterval> | undefined
  const startup = setTimeout(() => {
    void checkForUpdates(false)
    every = setInterval(() => void checkForUpdates(false), UPDATE_CHECK_INTERVAL_MS)
  }, STARTUP_CHECK_DELAY_MS)

  return () => {
    clearTimeout(startup)
    if (every !== undefined) {
      clearInterval(every)
    }
  }
}

/** Main window: silent update check on startup and the "Check for Updates…" menu item. */
export function useAutoUpdate(): void {
  useEffect(() => {
    return checkForUpdatesOnStartup()
  }, [])

  useAppCommand("check_for_updates", () => {
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
