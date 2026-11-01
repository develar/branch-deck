import type { GetReviewParams } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { chainTarget } from "@/lib/review-target"
import { loadFilesOfSubject } from "@/stores/commit-files"
import { loadFileDiffs } from "@/stores/file-diffs"
import { getDiffWhitespacePolicy } from "@/stores/settings/diff-view-settings"
import type { ReviewWindowData } from "@/types/sub-window"

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
 * The reads of the review window start with its page (`src/lib/review-window-start.ts` imports this module only in the
 * review window), beside the load of its route chunk (the diff engine, about 640 KB to parse), which they need nothing
 * of: of a list of commits the files and the diffs of its commit (into their stores, which the review then finds read or
 * being read), of a branch its segments (`readReview`)
 */
export function startReviewWindowReads(): void {
  const data = window.__INIT_DATA__ as ReviewWindowData | undefined
  if (data === undefined) {
    return
  }
  if (data.kind === "commits") {
    const id = data.commits[data.index]?.id
    if (id !== undefined) {
      const subject = { kind: "commit", id } as const
      void loadFilesOfSubject(subject)
      loadFileDiffs(subject, getDiffWhitespacePolicy(), { priorityPaths: data.path == null ? [] : [data.path] })
    }
  } else {
    const target = data.kept === undefined ? chainTarget(data.chain, data.wholeStack === true) : ({ kind: "kept", name: data.kept.name } as const)
    const params = { repositoryPath: data.repositoryPath, baselineBranch: data.baselineBranch, target }
    startedReview = { key: reviewKey(params), read: commands.getReview(params) }
  }
}
