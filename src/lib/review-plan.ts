import type { DeclarationTarget, ReviewedTip } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { ReviewPlace } from "@/lib/review-place"

/*
 * The review plan of a branch review window (spec/branch/review-plan.spec.md, Status: Experimental): the one module of
 * the page that calls `mark_branch_reviewed` and `get_outside_place`. Rust names the branch, takes its base and tip and writes the reviewed
 * pair with compare-and-swap (`mark_reviewed_core`, as `bd mark-reviewed`); the page only says a refusal.
 */

/**
 * Marks `branch` (`auth`, as the view names it) of `repositoryPath` reviewed at the base and the tip it has now; a branch
 * stacked on another passes the head of its parent as `base`, so that the reviewed commits are its own. A refusal is a
 * toast and the answer `null`
 */
export async function markBranchReviewed(repositoryPath: string, branch: string, base: string | null): Promise<ReviewedTip | null> {
  const result = await commands.markBranchReviewed({ repositoryPath, branch, base })
  if (result.status === "error") {
    notifyError(`Cannot mark ${branch} reviewed`, result.error)
    return null
  }
  return result.data
}

/**
 * A use of a changed API that the change left outside (`place`, its line from 1) as the view of its file at `revision`
 * opens it over the review (`get_outside_place`); a file or a line the revision lacks, or a failed read, is a toast and
 * the answer `null`
 */
export async function readOutsidePlace(repositoryPath: string, revision: string, place: ReviewPlace): Promise<DeclarationTarget | null> {
  const line = place.line ?? 1
  const result = await commands.getOutsidePlace({ repositoryPath, revision, path: place.path, line, name: place.name })
  if (result.status === "error") {
    notifyError(`Cannot open ${place.path}:${line}`, result.error)
    return null
  }
  if (result.data === null) {
    notifyError(`Cannot open ${place.path}:${line}`, "The head of the review has no such line")
  }
  return result.data
}

/**
 * The file of a use left outside at `revision`, for its excerpt in the story (`get_outside_place`): `null` when the
 * revision lacks the file or the line, or the read failed; no toast, the story shows the uses it read
 */
export async function readOutsideExcerpt(repositoryPath: string, revision: string, path: string, line: number, name: string): Promise<DeclarationTarget | null> {
  const result = await commands.getOutsidePlace({ repositoryPath, revision, path, line, name })
  return result.status === "ok" ? result.data : null
}
