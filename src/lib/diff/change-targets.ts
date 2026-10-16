import type { FileDiffMetadata } from "@pierre/diffs"

/** The first line of a block of changed lines: on the new side, or on the old side for a block that only deletes */
export interface ChangeTarget {
  side: "additions" | "deletions"
  lineNumber: number
}

/** The blocks of changed lines of a diff, top to bottom: what N and P step through in the review */
export function getChangeTargets(fileDiff: FileDiffMetadata): ChangeTarget[] {
  const targets: ChangeTarget[] = []
  for (const hunk of fileDiff.hunks) {
    let additionOffset = 0
    let deletionOffset = 0
    for (const content of hunk.hunkContent) {
      if (content.type === "context") {
        additionOffset += content.lines
        deletionOffset += content.lines
        continue
      }
      targets.push(
        content.additions > 0 ? { side: "additions", lineNumber: hunk.additionStart + additionOffset } : { side: "deletions", lineNumber: hunk.deletionStart + deletionOffset },
      )
      additionOffset += content.additions
      deletionOffset += content.deletions
    }
  }
  return targets
}
