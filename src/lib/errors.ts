import { error as logErrorToBackend } from "@tauri-apps/plugin-log"
import { toast } from "@/components/ui/toast"
import type { ToastActionProps } from "@/lib/toast-message"

/** The message of an error for the user: the `message` of an `Error` (no "Error:" prefix) or of a `CommandError` */
export function getErrorDetails(error: unknown): string {
  if (error instanceof Error) {
    return error.message
  }
  // CommandError returned by Tauri commands
  if (typeof error === "object" && error !== null && "message" in error && typeof error.message === "string") {
    return error.message
  }
  return String(error)
}

/** Records the errors the user was told about in Activity; set by `@/stores/activity`, which this module can't import */
let errorRecorder: ((title: string, description: string) => void) | null = null

export function setErrorRecorder(recorder: (title: string, description: string) => void): void {
  errorRecorder = recorder
}

/**
 * Logs the error and records it in Activity, without a toast: another window tells the user (the review window asked
 * for what failed). Returns the message for the user.
 */
export function recordError(title: string, error: unknown): string {
  const description = getErrorDetails(error)
  logError(title, error, description)
  errorRecorder?.(title, description)
  return description
}

/**
 * Logs the error, shows a sticky error toast and records it in Activity. Usable outside React. `action` opens the object
 * the error names ("Open in Patronus" of the robot that refused the push). The same error again while its toast is open
 * (a poll, two surfaces) updates that toast instead of stacking a copy; Activity records each.
 */
export function notifyError(title: string, error: unknown, action?: ToastActionProps): void {
  const description = recordError(title, error)

  toast.add({
    // Base UI updates the open toast of the same id in place
    id: `error:${title}\n${description}`,
    type: "error",
    title,
    description,
    // don't auto-dismiss errors
    timeout: 0,
    ...(action !== undefined && { actionProps: action }),
  })
}

export function logError(title: string, error: unknown, description: string | null = null): void {
  console.error(title, error)
  logErrorToBackend(title, { keyValues: { error: description ?? getErrorDetails(error) } }).catch((logFailure: unknown) => {
    console.error("Failed to forward error to backend log", logFailure)
  })
}
