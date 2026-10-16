/** The file of a review on screen: its path, its place in the order of the tree and the number of files */
export interface ReviewPosition {
  path: string | null
  index: number
  count: number
}

/**
 * The footer of a review: "File 3 / 12", after the commit ("Commit 2 / 5 · File 3 / 12") or the scope of a range
 * ("All Changes · File 3 / 12") when there is one; empty until the review reports its first file
 */
export function formatReviewPosition({ file, commit, scope }: { file: ReviewPosition | null, commit?: { index: number, count: number }, scope?: string }): string {
  const parts: string[] = []
  if (commit !== undefined) {
    parts.push(`Commit ${commit.index + 1} / ${commit.count}`)
  }
  else if (scope !== undefined) {
    parts.push(scope)
  }
  if (file !== null && file.count > 0) {
    parts.push(`File ${file.index + 1} / ${file.count}`)
  }
  return parts.join(" · ")
}
