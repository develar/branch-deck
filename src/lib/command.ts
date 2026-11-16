import { internalError } from "@/lib/api-error"
import type { ApiError } from "@/lib/bindings"
import { getErrorDetails, logWarning } from "@/lib/errors"

export type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: ApiError }

/** Runs a command; an IPC failure (the command did not run) becomes an error result as well. */
export async function invokeCommand<T>(run: () => Promise<CommandResult<T>>): Promise<CommandResult<T>> {
  try {
    return await run()
  } catch (error) {
    return { status: "error", error: internalError(getErrorDetails(error)) }
  }
}

/**
 * Runs a read ahead of what the user asks for next (`prefetch_review` as a review opens): a best-effort read, whose
 * failure is no failure of the user's, as the read they ask for tells its own. So a failure of the command or of the IPC
 * is a warning of the log, never an error line and never a rejection of the page. Resolves to whether it read
 */
export async function readAhead(what: string, run: () => Promise<CommandResult<unknown>>): Promise<boolean> {
  const result = await invokeCommand(run)
  if (result.status === "error") {
    logWarning(`Cannot read ${what} ahead`, result.error)
  }
  return result.status === "ok"
}
