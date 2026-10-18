import { getCurrentWindow } from "@tauri-apps/api/window"
import type { GetBranchReviewParams } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { CommitReviewWindowData } from "@/lib/commit-review-window"
import { loadFilesOfSubject } from "@/stores/commit-files"
import { loadSubjectPatch } from "@/stores/commit-patch"
import type { BranchReviewWindowData } from "@/types/sub-window"

type BranchReviewRead = ReturnType<typeof commands.getBranchReview>

/** The read of the review of a branch the page started at its load, for the first read of the page */
let startedBranchReview: { key: string; read: BranchReviewRead } | null = null

function branchReviewKey(params: GetBranchReviewParams): string {
  return JSON.stringify([params.repositoryPath, params.branchPrefix, params.baselineBranch, params.chain, params.wholeStack === true])
}

/** `get_branch_review`: the read the page started at its load when it is the same, once; otherwise a new one */
export function readBranchReview(params: GetBranchReviewParams): BranchReviewRead {
  const started = startedBranchReview
  startedBranchReview = null
  return started !== null && started.key === branchReviewKey(params) ? started.read : commands.getBranchReview(params)
}

/**
 * The reads of a review window start with its page, beside the load of its route chunk (the diff engine, about
 * 640 KB to parse), which they need nothing of: the files and the patch of the commit of the commit review (into their
 * stores, which the review then finds read or being read), the segments of the branch review (`readBranchReview`)
 */
export function startReviewWindowReads(): void {
  let label: string
  try {
    label = getCurrentWindow().label
  } catch {
    return
  }
  if (label === "commit-review") {
    const data = window.__INIT_DATA__ as CommitReviewWindowData | undefined
    const id = data?.commits[data.index]?.id
    if (id !== undefined) {
      const subject = { kind: "commit", id } as const
      void loadFilesOfSubject(subject)
      loadSubjectPatch(subject)
    }
  } else if (label === "branch-review") {
    const data = window.__INIT_DATA__ as BranchReviewWindowData | undefined
    if (data !== undefined) {
      const params = {
        repositoryPath: data.repositoryPath,
        branchPrefix: data.branchPrefix,
        baselineBranch: data.baselineBranch,
        chain: [...data.chain],
        wholeStack: data.wholeStack === true,
      }
      startedBranchReview = { key: branchReviewKey(params), read: commands.getBranchReview(params) }
    }
  }
}
