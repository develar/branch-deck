import { useCallback, useEffect, useState } from "react"
import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import type { MergeConflictInfo, UncommittedChangesResult } from "@/lib/bindings"
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
    return result.status === "ok"
      ? { diffData: result.data, diffLoading: false, diffError: null }
      : { diffData: null, diffLoading: false, diffError: result.error.message }
  }
  catch (error) {
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
  if (tipCommit === undefined) {
    toast.add({ type: "error", title: `${branch.name}: No Commits Found`, description: "Branch has no commits to amend changes to" })
    return
  }

  if (!diffData?.files) {
    toast.add({ type: "error", title: `${branch.name}: No Files to Amend`, description: "No uncommitted changes found to amend" })
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
      success: ({ amendedCommitId, rebasedToCommit }) => ({
        title: `${branch.name}: Changes Amended`,
        description: `Amended commit ${amendedCommitId.slice(0, 8)} → ${rebasedToCommit.slice(0, 8)}`,
        duration: 5000,
      }),
      error: (error) => {
        if (error instanceof AmendConflictError) {
          openInline("amend-conflict", branch.name, error.conflict)
          return { title: "Conflict detected", description: "Opening conflict resolver…" }
        }
        return {
          title: `${branch.name}: Failed to Amend Changes`,
          description: error instanceof Error ? error.message : "Failed to amend uncommitted changes",
        }
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
    void loadUncommittedChanges().then((loaded) => {
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
