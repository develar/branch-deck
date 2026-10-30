/**
 * A place in the diffs of a review that a link of the review plan shows (spec/branch/review-plan.spec.md): a changed
 * file by its last path or its old path, the side of the diff, the line from 1 of the declaration or the use (null
 * shows the file at its header), and the name there, which the diffs select
 */
export interface ReviewPlace {
  readonly path: string
  readonly side: "old" | "new"
  readonly line: number | null
  readonly name: string | null
}

/** A changed file at its header */
export function fileTop(path: string): ReviewPlace {
  return { path, side: "new", line: null, name: null }
}
