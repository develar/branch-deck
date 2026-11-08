/** The file of a review on screen: its path, its place in the order of the tree and the number of files */
export interface ReviewPosition {
  path: string | null
  index: number
  count: number
  /** The name of the chosen owner chip, without a second file count */
  owner?: string
  /** The overview of the plan is at the top: no file is, the footer says "Overview" */
  overview?: boolean
  /** The number of the step of the story at the top, from 1: "Step 2 · File 3 / 9" */
  step?: number
}

/**
 * The file a position shows the reader: none while the overview is at the top, though the tree keeps the first file for
 * its keys; a review read again starts at it, and the review of a pull request counts it as opened for Approve
 */
export function shownPath(position: ReviewPosition): string | null {
  return position.overview === true ? null : position.path
}

/**
 * The footer of a review: "File 3 / 12", after the commit ("Commit 2 / 5 · File 3 / 12") or the scope of a range
 * ("All Changes · File 3 / 12") when there is one, after the step of the story at the top ("Step 2 · File 3 / 12"),
 * then the name of the chosen owner chip; "Overview" in place of the file while the overview is at the top; empty until
 * the review reports its first file
 */
export function formatReviewPosition({ file, commit, scope }: { file: ReviewPosition | null; commit?: { index: number; count: number }; scope?: string }): string {
  const parts: string[] = []
  if (commit !== undefined) {
    parts.push(`Commit ${commit.index + 1} / ${commit.count}`)
  } else if (scope !== undefined) {
    parts.push(scope)
  }
  if (file?.overview === true) {
    parts.push("Overview")
  } else if (file !== null && file.count > 0) {
    if (file.step !== undefined) {
      parts.push(`Step ${file.step}`)
    }
    parts.push(`File ${file.index + 1} / ${file.count}`)
    if (file.owner !== undefined) {
      parts.push(file.owner)
    }
  }
  return parts.join(" · ")
}
