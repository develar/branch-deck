import { useCallback, useEffect, useState } from "react"
import type { PatronusMergeRequest, PatronusReviewRequirement } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { branchSyncStore } from "@/stores/branch-sync"
import { createPatronusMergeRequest, patronusRequest } from "@/stores/patronus"

export type ReviewRequirementState
  = | { status: "loading" }
    | { status: "loaded", requirement: PatronusReviewRequirement }
    | { status: "error", message: string }

export interface ReviewRequirement {
  state: ReviewRequirementState
  /** Asks Patronus again, not the cached answer: approvals may have arrived */
  recheck: () => void
  /** Opens the merge request of the branch on Space and checks again */
  createMergeRequest: () => Promise<PatronusMergeRequest>
  isCreatingMergeRequest: boolean
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/**
 * The code-owner review a Safe Push of the branch needs, asked when `enabled` (the Safe Push form is open, never for a
 * dry run): the heaviest call of Patronus, cached by the backend by the commits it checked.
 */
export function usePatronusReviewRequirement(branchName: string, enabled: boolean): ReviewRequirement {
  const [state, setState] = useState<ReviewRequirementState>({ status: "loading" })
  const [generation, setGeneration] = useState(0)
  const [isCreatingMergeRequest, setCreatingMergeRequest] = useState(false)

  useEffect(() => {
    if (!enabled) {
      return
    }
    let current = true
    const params = { ...patronusRequest(), baselineBranch: branchSyncStore.getState().baselineBranch ?? "", branchName, recheck: generation > 0 }
    commands.getPatronusReviewRequirement(params).then((result) => {
      if (current) {
        setState(result.status === "ok" ? { status: "loaded", requirement: result.data } : { status: "error", message: result.error.message })
      }
    }, (error: unknown) => {
      if (current) {
        setState({ status: "error", message: errorMessage(error) })
      }
    })
    return () => {
      current = false
    }
  }, [enabled, branchName, generation])

  const recheck = useCallback(() => {
    setState({ status: "loading" })
    setGeneration(it => it + 1)
  }, [])

  const createMergeRequest = useCallback(async () => {
    setCreatingMergeRequest(true)
    try {
      const mergeRequest = await createPatronusMergeRequest(branchName)
      recheck()
      return mergeRequest
    }
    finally {
      setCreatingMergeRequest(false)
    }
  }, [branchName, recheck])

  return { state, recheck, createMergeRequest, isCreatingMergeRequest }
}
