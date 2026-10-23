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
 * of the repository wait for it in the queue, and a start while another rewrite runs waits for that one the same way. A
 * failure is an error toast titled `failureTitle`. Resolves to the result of the command, null if it failed; a second
 * start of the same action resolves with the first.
 */
export async function runRewrite<T>(spec: RewriteSpec, failureTitle: string, command: () => Promise<CommandResult<T>>, applied: (data: T) => RewriteOutcome): Promise<T | null> {
  // a rewrite never resolves to `undefined`: one that must wait waits in the queue
  const value = await runOperation({ ...spec, kind: "rewrite" }, async () => {
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
  })
  return value ?? null
}

/**
 * Runs the undo of a rewrite (`runRewrite`; a command whose result is nothing) and resolves to whether it undid it: the
 * toast and the Activity entry of the rewrite offer their Undo again after a failure (`showUndoToast`)
 */
export async function runUndo<T>(spec: RewriteSpec, failureTitle: string, command: () => Promise<CommandResult<T>>, applied: () => void = () => {}): Promise<boolean> {
  let undone = false
  await runRewrite(spec, failureTitle, command, () => {
    undone = true
    applied()
    return { sync: true }
  })
  return undone
}
