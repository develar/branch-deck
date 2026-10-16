import { useCallback, useEffect, useState } from "react"
import type { PatronusReviewRequirement } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { rememberReviewGroups } from "@/lib/branch-review-window"
import { branchSyncStore, useBranchTip } from "@/stores/branch-sync"
import { patronusRequest } from "@/stores/patronus"

export type ReviewRequirementState = { status: "loading" } | { status: "loaded"; requirement: PatronusReviewRequirement } | { status: "error"; message: string }

export interface ReviewRequirement {
  state: ReviewRequirementState
  /** Asks Patronus again, not the cached answer: approvals may have arrived */
  recheck: () => void
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/**
 * The code-owner review a Safe Push of the branch needs, asked when `enabled` (the Safe Push form is open, never for a
 * dry run) and again when the local tip of the branch changes (an unstack leaves only its own commits): the heaviest
 * call of Patronus, cached by the backend by the commits it checked.
 */
export function usePatronusReviewRequirement(branchName: string, enabled: boolean): ReviewRequirement {
  const [generation, setGeneration] = useState(0)
  const localTip = useBranchTip(branchName)
  // an answer counts only for the commits it checked: after a tip change or a recheck the review is loading again, so
  // the form neither shows the groups of the old commits nor starts with their merge request
  const key = `${branchName}\n${localTip ?? ""}\n${generation}`
  const [answer, setAnswer] = useState<{ key: string; state: ReviewRequirementState } | null>(null)

  useEffect(() => {
    if (!enabled) {
      return
    }
    let current = true
    const answerWith = (state: ReviewRequirementState) => {
      if (current) {
        setAnswer({ key, state })
      }
    }
    const params = { ...patronusRequest(), baselineBranch: branchSyncStore.getState().baselineBranch ?? "", branchName, recheck: generation > 0 }
    commands.getPatronusReviewRequirement(params).then(
      result => {
        if (result.status === "ok") {
          // a review window opened from the row shows the groups too
          if (current) {
            rememberReviewGroups(branchName, result.data)
          }
          answerWith({ status: "loaded", requirement: result.data })
        } else {
          answerWith({ status: "error", message: result.error.message })
        }
      },
      (error: unknown) => answerWith({ status: "error", message: errorMessage(error) }),
    )
    return () => {
      current = false
    }
  }, [enabled, key, branchName, generation])

  const recheck = useCallback(() => setGeneration(it => it + 1), [])
  const state: ReviewRequirementState = answer?.key === key ? answer.state : { status: "loading" }
  return { state, recheck }
}
