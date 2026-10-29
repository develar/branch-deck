import { toast } from "@/components/ui/toast"
import type { Outcome } from "@/lib/bindings"
import { followUndo } from "@/model/store"
import { recordRewrite, UNDO_WINDOW_MS } from "@/stores/activity"

/**
 * Success toast of a history rewrite of `repositoryPath`, with an Undo button for 10 seconds; Activity records it in
 * that repository with the same Undo, naming the operation of the engine that ran it (`outcome.op`, its log lines).
 * `undo` resolves to whether it undid the rewrite: the toast closes then, and stays after an undo that failed or whose
 * wait for the lock the user stopped (a toast says why), so its Undo can be pressed again. The Undo lasts while the
 * session offers it (`followUndo`): once HEAD moved since (another rewrite, a commit, an update, a rewrite of `bd`) it
 * goes from the toast and from Activity, and the toast tells what was done until it closes. Returns the id of the toast,
 * so a check that answers later can add an action to it.
 */
export function showUndoToast(repositoryPath: string, title: string, undo: () => Promise<boolean>, description?: string, outcome?: Pick<Outcome, "op" | "versionAfter">): string {
  const id = toast.add({
    type: "success",
    title,
    description,
    timeout: UNDO_WINDOW_MS,
    actionProps: {
      children: "Undo",
      onClick: () => recorded.undo(),
    },
  })
  let running = false
  let withdrawn = false
  // whichever Undo comes first, the toast's or the one in Activity, undoes once and closes the toast once it undid
  const recorded = recordRewrite(
    repositoryPath,
    title,
    async () => {
      running = true
      const undone = await undo().finally(() => {
        running = false
      })
      if (undone) {
        stop()
        toast.close(id)
      } else if (withdrawn) {
        withdraw()
      }
      return undone
    },
    description,
    outcome?.op,
  )
  // the session took the Undo back: the toast and the entry lose it, but while it runs, which may be what took it back
  const withdraw = () => {
    withdrawn = true
    if (!running) {
      toast.update(id, { actionProps: undefined })
      recorded.withdraw()
    }
  }
  const unfollow = outcome === undefined ? () => {} : followUndo(repositoryPath, outcome, withdraw)
  const timer = setTimeout(unfollow, UNDO_WINDOW_MS)
  const stop = () => {
    unfollow()
    clearTimeout(timer)
  }
  return id
}
