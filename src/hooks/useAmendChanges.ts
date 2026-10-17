import { useCallback, useEffect, useState } from "react"
import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import type { MergeConflictInfo, UncommittedChangesResult } from "@/lib/bindings"
import { formatFileCount } from "@/lib/commit-files"
import type { Branch } from "@/stores/branch-sync"
import { syncBranches } from "@/stores/branch-sync"
import { openInline, withRowProcessing } from "@/stores/inline-row-action"
import { getSelectedProject } from "@/stores/repository"

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

/** Amends the uncommitted changes to the tip commit of the branch, then re-syncs. A conflict opens the inline conflict viewer. */
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
  const data = await withRowProcessing(
    branch.name,
    async () => {
      const result = await commands.amendUncommittedToBranch({
        repositoryPath: getSelectedProject()?.path ?? "",
        branchName: branch.name,
        originalCommitId: tipCommit.originalHash,
        files,
      })
      if (result.status !== "ok") {
        throw new Error(result.error.message)
      }
      if (result.data.status === "branchError") {
        const branchError = result.data.data
        if (branchError.MergeConflict) {
          throw new AmendConflictError(branchError.MergeConflict)
        }
        throw new Error(branchError.Generic)
      }
      return result.data.data
    },
    {
      processingMessage: `Amending changes to ${branch.name}…`,
      // the description is the amended commit
      success: () => ({ title: `Amended ${formatFileCount(files.length)} into ${branch.name}`, description: tipCommit.strippedSubject }),
      errorTitle: error => {
        if (error instanceof AmendConflictError) {
          // the conflict viewer below the row says it
          openInline("amend-conflict", branch.name, error.conflict)
          return null
        }
        return `Cannot amend ${branch.name}`
      },
    },
  )

  if (data) {
    await syncBranches({ autoScroll: false, autoExpand: false })
  }
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
