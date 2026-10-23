import { error as logErrorToBackend } from "@tauri-apps/plugin-log"
import { toast } from "@/components/ui/toast"
import type { CommandError } from "@/lib/bindings"
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

/**
 * The operation a failed command names (`CommandError.operation_id`, set by `operation`, `operation_now` or the one
 * `run_blocking` step of `src-tauri/src/operation.rs`): the id of its log lines, `operation{operation_id=…}`; undefined
 * for any other error
 */
export function getErrorOperationId(error: unknown): string | undefined {
  if (typeof error === "object" && error !== null && "operation_id" in error && typeof error.operation_id === "string") {
    return error.operation_id
  }
  return undefined
}

/**
 * The error of a failed command as an `Error` to throw, with what the page tells apart: its `kind` (a wait the user
 * stopped, `isLockWaitCanceled`) and its `operation_id`, which `notifyError` and `recordError` keep for Activity and the
 * log line. A step that throws what a command returned throws this, never `new Error(error.message)`, which drops both
 */
export function commandFailure(error: CommandError): Error {
  return Object.assign(new Error(error.message), { kind: error.kind ?? null, operation_id: error.operation_id ?? null })
}

type ErrorRecorder = (title: string, description: string, operationId: string | undefined) => void

/** Records the errors the user was told about in Activity; set by `@/stores/activity`, which this module can't import */
let errorRecorder: ErrorRecorder | null = null

export function setErrorRecorder(recorder: ErrorRecorder): void {
  errorRecorder = recorder
}

/**
 * Logs the error and records it in Activity, without a toast: another window tells the user (the review window asked
 * for what failed). Returns the message for the user.
 */
export function recordError(title: string, error: unknown): string {
  const description = getErrorDetails(error)
  logError(title, error, description)
  errorRecorder?.(title, description, getErrorOperationId(error))
  return description
}

/**
 * Logs the error, shows a sticky error toast and records it in Activity. Usable outside React. `action` opens the object
 * the error names ("Open in Patronus" of the robot that refused the push); `secondary` is a second action below it
 * ("Start Anyway"). The same error again while its toast is open (a poll, two surfaces) updates that toast instead of
 * stacking a copy; Activity records each.
 */
export function notifyError(title: string, error: unknown, action?: ToastActionProps, secondary?: { readonly label: string; readonly onClick: () => void }): void {
  const description = recordError(title, error)
  const id = `error:${title}\n${description}`

  toast.add({
    // Base UI updates the open toast of the same id in place
    id,
    type: "error",
    title,
    description,
    // don't auto-dismiss errors
    timeout: 0,
    ...(action !== undefined && { actionProps: action }),
    ...(secondary !== undefined && {
      data: {
        secondaryAction: {
          label: secondary.label,
          onClick: () => {
            toast.close(id)
            secondary.onClick()
          },
        },
      },
    }),
  })
}

export function logError(title: string, error: unknown, description: string | null = null): void {
  console.error(title, error)
  // the operation's id leads from this line to the lines the command logged
  const keyValues = { error: description ?? getErrorDetails(error), operation_id: getErrorOperationId(error) }
  logErrorToBackend(title, { keyValues }).catch((logFailure: unknown) => {
    console.error("Failed to forward error to backend log", logFailure)
  })
}
