import type { ReviewedTip } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"

/*
 * The review plan of a branch review window (spec/branch/review-plan.spec.md, Status: Experimental): the one module of
 * the page that calls `mark_branch_reviewed`. Rust names the branch, takes its base and tip and writes the reviewed
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
