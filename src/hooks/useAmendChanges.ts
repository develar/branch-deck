import { useCallback, useEffect, useState } from "react"
import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import type { MergeConflictInfo, UncommittedChangesResult } from "@/lib/bindings"
import { formatFileCount } from "@/lib/commit-files"
import { commandFailure } from "@/lib/errors"
import type { Branch } from "@/stores/branch-sync"
import { resolveCommitsOf, syncBranches } from "@/stores/branch-sync"
import { openInline, withRowProcessing } from "@/stores/inline-row-action"
import { getSelectedProject, getSelectedRepositoryPath, isSelectedRepository } from "@/stores/repository"

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

class AmendConflictError extends Error {
  constructor(readonly conflict: MergeConflictInfo) {
    super("Merge conflict")
  }
}

/**
 * Amends the uncommitted changes to the tip commit of the branch, then re-syncs: one operation. A conflict opens the
 * inline conflict viewer. An amend that waits for another rewrite in the queue runs in the repository of its start, on
 * the commit as that rewrite left it.
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
  // the repository of the start: an amend that waits in the queue runs there, whichever the main window shows by then
  const repositoryPath = getSelectedRepositoryPath()
  await withRowProcessing(
    branch.name,
    // an amend of other files is an amend of its own: it waits for this one, never joins it
    { id: "amend-changes", kind: "rewrite", parameters: JSON.stringify(files), label: `Amending changes to ${branch.name}`, repositoryPath },
    async wait => {
      const result = await commands.amendUncommittedToBranch(
        {
          repositoryPath,
          branchName: branch.name,
          // the id as the rewrite before it in the queue left it (`resolveCommitsOf`)
          originalCommitId: resolveCommitsOf(repositoryPath, [tipCommit])[0]?.originalHash ?? tipCommit.originalHash,
          files,
        },
        wait,
      )
      if (result.status !== "ok") {
        // the error as the command answered it: a wait the user stopped is no failure (`isLockWaitCanceled`)
        throw commandFailure(result.error)
      }
      if (result.data.status === "branchError") {
        const branchError = result.data.data
        if (branchError.MergeConflict) {
          throw new AmendConflictError(branchError.MergeConflict)
        }
        throw new Error(branchError.Generic)
      }
      // the table shows another repository after a switch: this one is synced when it opens
      if (isSelectedRepository(repositoryPath)) {
        await syncBranches({ autoScroll: false, autoExpand: false })
      }
      return result.data.data
    },
    {
      // the description is the amended commit
      success: () => ({ title: `Amended ${formatFileCount(files.length)} into ${branch.name}`, description: tipCommit.strippedSubject }),
      errorTitle: error => {
        if (error instanceof AmendConflictError) {
          // the conflict viewer below the row says it
          openInline("amend-conflict", branch.name, { conflictInfo: error.conflict })
          return null
        }
        return `Cannot amend ${branch.name}`
      },
    },
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
