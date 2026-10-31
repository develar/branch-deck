/** The file of a review on screen: its path, its place in the order of the tree and the number of files */
export interface ReviewPosition {
  path: string | null
  index: number
  count: number
  /**
   * The part of the chosen owner chip that marks files: its name, the place of the file among the owner's files in the
   * order of the tree (-1 for a file of another owner) and their number; none without such a chip
   */
  owner?: { label: string; index: number; count: number }
}

/**
 * The footer of a review: "File 3 / 12", after the commit ("Commit 2 / 5 · File 3 / 12") or the scope of a range
 * ("All Changes · File 3 / 12") when there is one, then the part of the chosen owner chip: "Design 2 / 5" on a file of
 * its own, "Design 5" on another; empty until the review reports its first file
 */
export function formatReviewPosition({ file, commit, scope }: { file: ReviewPosition | null; commit?: { index: number; count: number }; scope?: string }): string {
  const parts: string[] = []
  if (commit !== undefined) {
    parts.push(`Commit ${commit.index + 1} / ${commit.count}`)
  } else if (scope !== undefined) {
    parts.push(scope)
  }
  if (file !== null && file.count > 0) {
    parts.push(`File ${file.index + 1} / ${file.count}`)
    const owner = file.owner
    if (owner !== undefined && owner.count > 0) {
      parts.push(owner.index < 0 ? `${owner.label} ${owner.count}` : `${owner.label} ${owner.index + 1} / ${owner.count}`)
    }
  }
  return parts.join(" · ")
}
