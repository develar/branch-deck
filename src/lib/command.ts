import { toast } from "@/components/ui/toast"
import type { CommandError } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"
import { recordRewrite, UNDO_WINDOW_MS } from "@/stores/activity"

export type CommandResult<T> = { status: "ok", data: T } | { status: "error", error: CommandError }

/** Runs a command; an IPC failure (the command did not run) becomes an error result as well. */
export async function invokeCommand<T>(run: () => Promise<CommandResult<T>>): Promise<CommandResult<T>> {
  try {
    return await run()
  }
  catch (error) {
    return { status: "error", error: { message: getErrorDetails(error) } }
  }
}

/** Success toast of a history rewrite, with an Undo button for 10 seconds; Activity records it with the same Undo. */
export function showUndoToast(title: string, undo: () => Promise<void>, description?: string): void {
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
  // whichever Undo comes first, the toast's or the one in Activity, undoes once and closes the toast
  const runUndo = recordRewrite(title, () => {
    toast.close(id)
    return undo()
  }, description)
}
