import { useCallback, useEffect, useState } from "react"
import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import type { UncommittedChangesResult } from "@/lib/bindings"
import { formatFileCount } from "@/lib/commit-files"
import { notifyError } from "@/lib/errors"
import { runRewriteIntent, witnessed } from "@/lib/rewrite-run"
import { showToast } from "@/lib/toast-message"
import type { Branch } from "@/stores/branch-sync"
import { inlineSurface, openInline, withRowProgress } from "@/stores/inline-row-action"
import { branchSubject } from "@/stores/operations"
import { getSelectedProject, getSelectedRepositoryPath } from "@/stores/repository"

interface UncommittedChangesState {
  diffData: UncommittedChangesResult | null
  diffLoading: boolean
  diffError: string | null
}

async function loadUncommittedChanges(): Promise<UncommittedChangesState> {
  try {
    const result = await commands.getUncommittedChanges({ repositoryPath: getSelectedProject()?.path ?? "" })
    return result.status === "ok" ? { diffData: result.data, diffLoading: false, diffError: null } : { diffData: null, diffLoading: false, diffError: result.error.message }
  } catch (error) {
    return { diffData: null, diffLoading: false, diffError: error instanceof Error ? error.message : "Failed to load uncommitted changes" }
  }
}

/**
 * Amends the uncommitted changes to the tip commit of the branch, as the rewrite `amend` of the engine
 * (`runRewriteIntent`) in the repository of the press, then the sync that shows it, while the inline form shows its
 * progress. A conflict opens the inline conflict viewer in place of the form; an error keeps the form, and a wait the
 * user stopped too. An amend that waits for another rewrite in the queue of the session runs in the repository of its
 * press, on the commit as that rewrite left it (the session follows its id).
 */
export async function amendChanges(branch: Branch, diffData: UncommittedChangesResult | null): Promise<void> {
  // the tip commit is the first one (newest, after .rev() in the backend)
  const tipCommit = branch.commits[0]
  // nothing to do: not an error
  if (tipCommit === undefined) {
    toast.add({ type: "info", title: "No commit to amend into", description: `${branch.name} has no commits.` })
    return
  }

  if (!diffData?.files || diffData.files.length === 0) {
    toast.add({ type: "info", title: "No changes to amend", description: "The working tree has no uncommitted changes." })
    return
  }

  const files = diffData.files.map(file => file.filePath)
  // the repository of the press: an amend that waits in the queue runs there, whichever the main window shows by then
  const repositoryPath = getSelectedRepositoryPath()
  await withRowProgress(
    branch.name,
    async () => {
      const done = await runRewriteIntent(
        {
          id: "amend-changes",
          subject: branchSubject(branch.name),
          objects: [branch.name],
          // an amend of other files is an amend of its own: it waits for this one, never joins it
          parameters: JSON.stringify(files),
          label: `Amending changes to ${branch.name}`,
          surface: inlineSurface(branch.name),
          repositoryPath,
        },
        witnessed(repositoryPath, { rewrite: "amend", branchName: branch.name, originalCommitId: tipCommit.originalHash, files }),
      )
      if (done === null || done.result.rewrite !== "amended") {
        return false
      }
      const answer = done.result.answer
      if (answer.status === "branchError") {
        if (answer.data.MergeConflict === undefined) {
          notifyError(`Cannot amend ${branch.name}`, answer.data.Generic)
        } else {
          // the conflict viewer below the row says it, in place of the form (a failure keeps the form: no close follows)
          openInline("amend-conflict", branch.name, { conflictInfo: answer.data.MergeConflict })
        }
        return false
      }
      // the description is the amended commit
      showToast({ title: `Amended ${formatFileCount(files.length)} into ${branch.name}`, description: tipCommit.strippedSubject })
      return true
    },
    { keepFormOnFailure: true },
  )
}

/** Uncommitted changes (loaded once on mount) and the amend action using them. */
export function useAmendChanges() {
  const [state, setState] = useState<UncommittedChangesState>({ diffData: null, diffLoading: true, diffError: null })

  useEffect(() => {
    let cancelled = false
    void loadUncommittedChanges().then(loaded => {
      if (!cancelled) {
        setState(loaded)
      }
    })
    return () => {
      cancelled = true
    }
  }, [])

  const { diffData } = state
  const amend = useCallback((branch: Branch) => amendChanges(branch, diffData), [diffData])

  return { ...state, amendChanges: amend }
}
