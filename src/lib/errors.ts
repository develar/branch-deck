import { error as logErrorToBackend } from "@tauri-apps/plugin-log"
import { toast } from "@/components/ui/toast"
import { getErrorDetails, isApiError, writeWarning } from "@/lib/api-error"
import type { ApiError, ErrorCode } from "@/lib/bindings"
import type { ToastActionProps } from "@/lib/toast-message"
import { presentLockHolder, presentOperationHolder } from "@/presenters/lock-wait"

/**
 * The code of the `ApiError` a command answered; undefined for any other error. The page acts on the code, never on the
 * words of an error (`pnpm lint:arch`)
 */
export function getErrorCode(error: unknown): ErrorCode | undefined {
  return isApiError(error) ? error.code : undefined
}

/**
 * The error in the words of the page, by its code: a held repository lock names its holder from `details`
 * ("`brd push auth` (pid 4242) has held the repository for 40 s"), never the lock file; an operation of another process
 * that holds what this one needs names it ("`bd` (pid 4242) has run the Safe Push of auth for 12 s"); any other error is
 * its message (`getErrorDetails`)
 */
export function describeError(error: unknown): string {
  if (isApiError(error) && error.code === "locked" && error.details?.type === "locked") {
    return error.details.holder === null ? "Another Branch Deck operation holds the repository" : presentLockHolder(error.details.holder)
  }
  if (isApiError(error) && error.code === "busy" && error.details?.type === "busy") {
    return error.details.holder === null ? "An operation of another version of Branch Deck runs: retry when it is done" : presentOperationHolder(error.details.holder)
  }
  return getErrorDetails(error)
}

/**
 * The operation a failed command names (`ApiError.operation_id`, set by `operation`, `operation_now` or the one
 * `run_blocking` step of `crates/app-api/src/operation.rs`): the id of its log lines, `operation{operation_id=…}`; undefined
 * for any other error
 */
export function getErrorOperationId(error: unknown): string | undefined {
  if (typeof error === "object" && error !== null && "operation_id" in error && typeof error.operation_id === "string") {
    return error.operation_id
  }
  return undefined
}

/**
 * The `ApiError` of a failed command as an `Error` to throw, with what the page tells apart: its `code` (a wait the user
 * stopped is `canceled`, `getErrorCode`), its data and its `operation_id`, which `notifyError` and `recordError`
 * keep for Activity and the log line. A step that throws what a command returned throws this, never
 * `new Error(error.message)`, which drops them
 */
export function commandFailure(error: ApiError): Error & ApiError {
  return Object.assign(new Error(error.message), error)
}

type ErrorRecorder = (title: string, description: string, operationId: string | undefined) => void

/** Records the errors the user was told about in Activity; set by `@/stores/activity`, which this module can't import */
let errorRecorder: ErrorRecorder | null = null

export function setErrorRecorder(recorder: ErrorRecorder): void {
  errorRecorder = recorder
}

/**
 * Logs the error and records it in Activity, without a toast: another window tells the user (the review window asked
 * for what failed). Returns the words for the user (`describeError`).
 */
export function recordError(title: string, error: unknown): string {
  const description = describeError(error)
  // the log keeps the backend's own words
  logError(title, error)
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
  // by the backend's words, which say no time: a retry refused by the same holder updates its toast
  const id = `error:${title}\n${getErrorDetails(error)}`

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

/**
 * Logs a warning: no failure of the app, a thing of the user's own the app cannot use (a pattern of theirs that is no
 * regex), a read ahead that failed (`readAhead` of src/lib/command.ts), which the read asked for tells again, or a
 * failed call of a command the page made in passing (the end of a subscription as its window leaves, the stop of a
 * search, a read of a status the next read tells again)
 */
export function logWarning(title: string, detail: unknown): void {
  writeWarning(title, detail)
}

export function logError(title: string, error: unknown): void {
  console.error(title, error)
  // the operation's id leads from this line to the lines the command logged
  const keyValues = { error: getErrorDetails(error), operation_id: getErrorOperationId(error) }
  logErrorToBackend(title, { keyValues }).catch((logFailure: unknown) => {
    console.error("Failed to forward error to backend log", logFailure)
  })
}
