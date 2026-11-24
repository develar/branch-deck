import { useEffect, useState } from "react"
import type { ShowReviewers } from "@/lib/bindings"
import { describeError } from "@/lib/errors"
import { readShowReviewers } from "@/lib/shown-review"
import { getSelectedRepositoryPath } from "@/stores/repository"

/** The default reviewers of a review after landing as the picker has them: asked, answered, or why not */
export type ShowReviewersState =
  | { readonly kind: "loading" }
  | { readonly kind: "loaded"; readonly value: ShowReviewers }
  /** The words of the failure: no Space token, the service or GitHub failed; the user adds the reviewers by hand */
  | { readonly kind: "failed"; readonly message: string }

const LOADING: ShowReviewersState = { kind: "loading" }

/**
 * Whom the review after landing of `branchName` asks by default (`readShowReviewers`), asked once per branch while
 * `enabled` (the popover of Safe Push and Request Review…), in the repository the window shows as it asks
 */
export function useShowReviewers(branchName: string, enabled: boolean): ShowReviewersState | null {
  const [answer, setAnswer] = useState<{ readonly branchName: string; readonly state: ShowReviewersState } | null>(null)
  useEffect(() => {
    if (!enabled) {
      return
    }
    let current = true
    // the repository of the ask: one the window shows later answers for its own branch
    const repositoryPath = getSelectedRepositoryPath()
    readShowReviewers(repositoryPath, branchName).then(
      value => current && setAnswer({ branchName, state: { kind: "loaded", value } }),
      (error: unknown) => current && setAnswer({ branchName, state: { kind: "failed", message: describeError(error) } }),
    )
    return () => {
      current = false
    }
  }, [branchName, enabled])
  // loading until the answer of this branch came
  return enabled && answer?.branchName === branchName ? answer.state : enabled ? LOADING : null
}
