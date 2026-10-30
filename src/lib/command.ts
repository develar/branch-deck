import { internalError } from "@/lib/api-error"
import type { ApiError } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"

export type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: ApiError }

/** Runs a command; an IPC failure (the command did not run) becomes an error result as well. */
export async function invokeCommand<T>(run: () => Promise<CommandResult<T>>): Promise<CommandResult<T>> {
  try {
    return await run()
  } catch (error) {
    return { status: "error", error: internalError(getErrorDetails(error)) }
  }
}
