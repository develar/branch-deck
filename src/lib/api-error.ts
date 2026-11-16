// no import but of types: the bindings import this module (`commandError`), and every module imports the bindings
import type { ApiError } from "@/lib/bindings"

/**
 * The `ApiError` of a failure without a code of its own (`internal`), as a command answers one: for an IPC call that
 * failed before the command ran, and for a result that stands for no answer yet
 */
export function internalError(message: string): ApiError {
  return { code: "internal", message, details: null, retry: "no", operation_id: null }
}

/** Whether `error` is the `ApiError` a command answers (`code`, `message`, …), not an `Error` of the page */
export function isApiError(error: unknown): error is ApiError {
  return typeof error === "object" && error !== null && "code" in error && typeof error.code === "string" && "message" in error && typeof error.message === "string"
}

/** The message of an error for the user: the `message` of an `Error` (no "Error:" prefix) or of an `ApiError` */
export function getErrorDetails(error: unknown): string {
  if (error instanceof Error) {
    return error.message
  }
  // the ApiError a Tauri command returns
  if (typeof error === "object" && error !== null && "message" in error && typeof error.message === "string") {
    return error.message
  }
  return String(error)
}

/**
 * The error of a failed call of a command, as its result holds it (the catch of `typedError` in the bindings, which
 * `src-tauri/src/bindings_test.rs` writes): the `ApiError` the command answered as is; any other failure, a refusal of
 * Tauri itself ("Command not found", an ACL refusal, invalid arguments: a string) or an `Error` of the page's IPC (a
 * fetch of the E2E bridge, the Air shim), an `internal` error of its words. So a command that answers a `Result` never
 * rejects
 */
export function commandError(error: unknown): ApiError {
  return isApiError(error) ? error : internalError(getErrorDetails(error))
}
