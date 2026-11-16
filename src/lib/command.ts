import { commandError } from "@/lib/api-error"
import type { ApiError } from "@/lib/bindings"
import { logWarning } from "@/lib/errors"

/** What a command that answers a `Result` resolves to: never a rejection, as the bindings answer every failure (`commandError`) */
export type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: ApiError }

/**
 * Runs a read ahead of what the user asks for next (`prefetch_review` as a review opens): a best-effort read, whose
 * failure is no failure of the user's, as the read they ask for tells its own. So a failure of the read is a warning of
 * the log, never an error line and never a rejection of the page: an error the command answered, and a rejection of
 * `run` itself, which may run a step of the page before the command. Resolves to whether it read
 */
export async function readAhead(what: string, run: () => Promise<CommandResult<unknown>>): Promise<boolean> {
  let result: CommandResult<unknown>
  try {
    result = await run()
  } catch (error) {
    result = { status: "error", error: commandError(error) }
  }
  if (result.status === "error") {
    logWarning(`Cannot read ${what} ahead`, result.error)
  }
  return result.status === "ok"
}
