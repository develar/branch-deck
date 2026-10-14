import { toast } from "@/components/ui/toast"
import type { CommandError } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"

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

/** Success toast of a history rewrite, with an Undo button for 10 seconds. */
export function showUndoToast(title: string, undo: () => Promise<void>): void {
  const id = toast.add({
    type: "success",
    title,
    timeout: 10_000,
    actionProps: {
      children: "Undo",
      onClick: () => {
        toast.close(id)
        void undo()
      },
    },
  })
}
