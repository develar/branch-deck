import { toast } from "@/components/ui/toast"
import type { CommandResult } from "@/lib/command"
import { invokeCommand } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { syncBranches } from "@/stores/branch-sync"
import type { OperationSpec } from "@/stores/operations"
import { runOperation } from "@/stores/operations"
import { isSelectedRepository } from "@/stores/repository"

/** A rewrite: its operation, without the kind */
export type RewriteSpec = Omit<OperationSpec, "kind">

/** What a rewrite did, as `applied` tells it after the command */
export interface RewriteOutcome {
  /** The branches show something new: the operation lasts until the sync shows it; false when nothing changed */
  readonly sync: boolean
  /** Runs once the sync showed the result: the toast with Undo, which the user sees beside what it undoes */
  readonly after?: () => void
}

/**
 * Runs a rewrite of the repository (a regroup, a squash, a reword, an undo) as one operation: the command, then the sync
 * that shows its result, then the toast `applied` says; its row is busy until then (`runOperation`), the other rewrites
 * of the repository wait. A failure is an error toast titled `failureTitle`; a start while another rewrite runs says so
 * under the same title and runs nothing. Resolves to the result of the command, null if it failed or did not run; a
 * second start of the same action resolves with the first.
 */
export async function runRewrite<T>(spec: RewriteSpec, failureTitle: string, command: () => Promise<CommandResult<T>>, applied: (data: T) => RewriteOutcome): Promise<T | null> {
  const value = await runOperation(
    { ...spec, kind: "rewrite" },
    async () => {
      const result = await invokeCommand(command)
      if (result.status !== "ok") {
        notifyError(failureTitle, result.error)
        return null
      }
      const outcome = applied(result.data)
      // the sync shows the selected repository: a rewrite of another one (an Undo clicked after a switch) is synced
      // when it opens
      if (outcome.sync && (spec.repositoryPath === undefined || isSelectedRepository(spec.repositoryPath))) {
        await syncBranches({ autoScroll: false, autoExpand: false })
      }
      outcome.after?.()
      return result.data
    },
    reason => toast.add({ type: "info", title: failureTitle, description: reason }),
  )
  return value ?? null
}
