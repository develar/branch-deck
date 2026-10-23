import type { ApiError } from "@/lib/bindings"

/**
 * The `ApiError` of a failure without a code of its own (`internal`), as a command answers one: for an IPC call that
 * failed before the command ran, and for a result that stands for no answer yet
 */
export function internalError(message: string): ApiError {
  return { code: "internal", message, details: null, retry: "no", operation_id: null }
}
