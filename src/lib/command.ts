import { toast } from "@/components/ui/toast"
import { internalError } from "@/lib/api-error"
import type { ApiError } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"
import { recordRewrite, UNDO_WINDOW_MS } from "@/stores/activity"

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
 * Success toast of a history rewrite of `repositoryPath`, with an Undo button for 10 seconds; Activity records it in
 * that repository with the same Undo.
 * `undo` resolves to whether it undid the rewrite: the toast closes then, and stays after an undo that failed (an error
 * toast says why), so its Undo can be pressed again. Returns the id of the toast, so a check that answers later can add
 * an action to it.
 */
export function showUndoToast(repositoryPath: string, title: string, undo: () => Promise<boolean>, description?: string): string {
  const id = toast.add({
    type: "success",
    title,
    description,
    timeout: UNDO_WINDOW_MS,
    actionProps: {
      children: "Undo",
      onClick: () => runUndo(),
    },
  })
  // whichever Undo comes first, the toast's or the one in Activity, undoes once and closes the toast once it undid
  const runUndo = recordRewrite(
    repositoryPath,
    title,
    async () => {
      const undone = await undo()
      if (undone) {
        toast.close(id)
      }
      return undone
    },
    description,
  )
  return id
}
