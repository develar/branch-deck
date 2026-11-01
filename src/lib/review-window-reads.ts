import type { GetReviewParams } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { CommitReviewWindowData } from "@/lib/commit-review-window"
import { chainTarget } from "@/lib/review-target"
import { BRANCH_REVIEW_WINDOW, COMMIT_REVIEW_WINDOW } from "@/lib/window-label"
import { loadFilesOfSubject } from "@/stores/commit-files"
import { loadFileDiffs } from "@/stores/file-diffs"
import { getDiffWhitespacePolicy } from "@/stores/settings/diff-view-settings"
import type { BranchReviewWindowData } from "@/types/sub-window"

type ReviewRead = ReturnType<typeof commands.getReview>

/** The read of the review the page started at its load, for the first read of the page */
let startedReview: { key: string; read: ReviewRead } | null = null

function reviewKey(params: GetReviewParams): string {
  return JSON.stringify([params.repositoryPath, params.baselineBranch, params.target])
}

/** `get_review`: the read the page started at its load when it is the same, once; otherwise a new one */
export function readReview(params: GetReviewParams): ReviewRead {
  const started = startedReview
  startedReview = null
  return started !== null && started.key === reviewKey(params) ? started.read : commands.getReview(params)
}

/**
 * The reads of a review window start with its page (`src/lib/review-window-start.ts` imports this module only in a
 * review window), beside the load of its route chunk (the diff engine, about 640 KB to parse), which they need nothing
 * of: the files and the diffs of the commit of the commit review (into their stores, which the review then finds read
 * or being read), the segments of the branch review (`readReview`); `label` is that of the window
 */
export function startReviewWindowReads(label: string): void {
  if (label === COMMIT_REVIEW_WINDOW) {
    const data = window.__INIT_DATA__ as CommitReviewWindowData | undefined
    const id = data?.commits[data.index]?.id
    if (id !== undefined) {
      const subject = { kind: "commit", id } as const
      void loadFilesOfSubject(subject)
      loadFileDiffs(subject, getDiffWhitespacePolicy(), { priorityPaths: data?.path == null ? [] : [data.path] })
    }
  } else if (label === BRANCH_REVIEW_WINDOW) {
    const data = window.__INIT_DATA__ as BranchReviewWindowData | undefined
    if (data !== undefined) {
      const target = data.kept === undefined ? chainTarget(data.chain, data.wholeStack === true) : ({ kind: "kept", name: data.kept.name } as const)
      const params = { repositoryPath: data.repositoryPath, baselineBranch: data.baselineBranch, target }
      startedReview = { key: reviewKey(params), read: commands.getReview(params) }
    }
  }
}
