import { useCallback, useEffect, useEffectEvent, useState } from "react"
import type { PatronusReviewRequirement, PullRequest, ReviewReadiness, ReviewReadinessBranch } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { rememberReviewGroups } from "@/lib/branch-review-window"
import { branchSyncStore, useBranchTip } from "@/stores/branch-sync"
import { patronusRequest } from "@/stores/patronus"

export type ReviewRequirementState =
  | { status: "loading" }
  | { status: "loaded"; requirement: PatronusReviewRequirement; readiness: ReviewReadiness }
  | { status: "error"; message: string }

export interface ReviewRequirement {
  state: ReviewRequirementState
  /** Asks Patronus again, not the kept answer: approvals may have arrived */
  recheck: () => void
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

/** The branch as the readiness command takes it: the robot starts from `pullRequest`, else through the API */
function readinessBranch(branchName: string, tip: string, pullRequest: PullRequest | null): ReviewReadinessBranch {
  if (pullRequest === null) {
    return { branchName, tip, start: { kind: "rest" } }
  }
  return { branchName, tip, start: { kind: "pullRequest", number: pullRequest.number }, reviewers: pullRequest.reviewers, author: pullRequest.author }
}

/** What an answer is about: the commits of the tip, and the reviews of the pull request it starts from */
function reviewsKey(pullRequest: PullRequest | null): string {
  return pullRequest === null ? "" : JSON.stringify([pullRequest.number, pullRequest.author, pullRequest.reviewers])
}

/**
 * The code-owner review a Safe Push of the branch needs, asked when `enabled` (the Safe Push form is open, never for a
 * dry run) and again when the local tip of the branch changes (an unstack leaves only its own commits) or a review of
 * `pullRequest` does, the pull request the robot would start from (null: a start through the API, which counts no
 * approval of one). Patronus judges the approvals; the backend keeps its answer by what it decides on.
 */
export function usePatronusReviewRequirement(branchName: string, enabled: boolean, pullRequest: PullRequest | null): ReviewRequirement {
  const [generation, setGeneration] = useState(0)
  const localTip = useBranchTip(branchName)
  // an answer counts only for the commits and the reviews it checked: after a change or a recheck the review is loading
  // again, so the form neither shows the groups of the old commits nor starts with their merge request
  const key = `${branchName}\n${localTip ?? ""}\n${reviewsKey(pullRequest)}\n${generation}`
  const [answer, setAnswer] = useState<{ key: string; state: ReviewRequirementState } | null>(null)
  // the pull request is in `key` by its reviews: a refresh that brings the same ones asks nothing
  const branchOf = useEffectEvent((tip: string) => readinessBranch(branchName, tip, pullRequest))

  useEffect(() => {
    if (!enabled || localTip === undefined) {
      return
    }
    let current = true
    const answerWith = (state: ReviewRequirementState) => {
      if (current) {
        setAnswer({ key, state })
      }
    }
    const params = { ...patronusRequest(), baselineBranch: branchSyncStore.getState().baselineBranch ?? "", branches: [branchOf(localTip)], recheck: generation > 0 }
    commands.getBranchReviewReadiness(params).then(
      result => {
        if (result.status === "error") {
          answerWith({ status: "error", message: result.error.message })
          return
        }
        const answer = result.data[0]
        if (answer?.requirement == null) {
          answerWith({ status: "error", message: answer?.readiness.kind === "unavailable" ? answer.readiness.message : "Patronus gave no answer" })
          return
        }
        // a review window opened from the row shows the groups too
        if (current) {
          rememberReviewGroups(branchName, answer.requirement)
        }
        answerWith({ status: "loaded", requirement: answer.requirement, readiness: answer.readiness })
      },
      (error: unknown) => answerWith({ status: "error", message: errorMessage(error) }),
    )
    return () => {
      current = false
    }
  }, [enabled, key, branchName, generation, localTip])

  const recheck = useCallback(() => setGeneration(it => it + 1), [])
  const state: ReviewRequirementState = answer?.key === key ? answer.state : { status: "loading" }
  return { state, recheck }
}
