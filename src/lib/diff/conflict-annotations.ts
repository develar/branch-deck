import type { DiffLineAnnotation } from "@pierre/diffs"
import type { ConflictCommit } from "@/lib/bindings"

export type ConflictMarkerType = "conflict-start" | "conflict-base" | "conflict-separator" | "conflict-end"

export interface ConflictMarkerInfo {
  type: ConflictMarkerType
  tooltip: string
  branch?: string
  base?: string
  commit?: string
}

/** The annotation of a diff line holding a conflict marker */
export interface ConflictLineData {
  tooltip: string
  conflictInfo?: ConflictMarkerInfo
  conflictMarkerCommits?: Record<string, ConflictCommit>
}

/** Extracts the marker kind and the referenced branch/commit from a line with a conflict marker */
export function extractConflictInfo(line: string): ConflictMarkerInfo | null {
  if (line.includes("<<<<<<< ")) {
    const branch = line.replace("<<<<<<< ", "").trim()
    return { type: "conflict-start", branch, tooltip: `Current branch: ${branch}` }
  }
  if (line.includes("||||||| ")) {
    const base = line.replace("||||||| ", "").trim()
    return { type: "conflict-base", base, tooltip: `Common ancestor: ${base}` }
  }
  if (line.includes("=======")) {
    return { type: "conflict-separator", tooltip: "Conflict separator" }
  }
  if (line.includes(">>>>>>> ")) {
    const commit = line.replace(">>>>>>> ", "").trim()
    return { type: "conflict-end", commit, tooltip: `Incoming commit: ${commit}` }
  }
  return null
}

/** An annotation under every added line of `patch` that holds a conflict marker, at its new-file line number */
export function getConflictAnnotations(patch: string, conflictMarkerCommits: Record<string, ConflictCommit> | undefined): DiffLineAnnotation<ConflictLineData>[] {
  const annotations: DiffLineAnnotation<ConflictLineData>[] = []
  let newFileLineNumber = 0
  let inHunk = false

  for (const line of patch.split("\n")) {
    // the hunk header gives the starting line number
    if (line.startsWith("@@")) {
      const match = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line)
      if (match?.[1]) {
        newFileLineNumber = Number.parseInt(match[1], 10)
        inHunk = true
      }
      continue
    }

    if (!inHunk) {
      continue
    }

    if (line.startsWith("+")) {
      const conflictInfo = extractConflictInfo(line.substring(1))
      if (conflictInfo) {
        annotations.push({ side: "additions", lineNumber: newFileLineNumber, metadata: { tooltip: conflictInfo.tooltip, conflictInfo, conflictMarkerCommits } })
      }
      newFileLineNumber++
    }
    // a deleted line and "\ No newline at end of file" don't advance the new file line number
    else if (!line.startsWith("-") && !line.startsWith("\\")) {
      newFileLineNumber++
    }
  }

  return annotations
}
