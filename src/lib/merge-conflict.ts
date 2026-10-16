import type { ConflictAnalysis, FileDiff } from "@/lib/bindings"
import { getSubjectLine, parseSubject } from "@/lib/commit"

/** The common ancestor of the conflicting commit and its target */
export interface MergeBase {
  hash: string
  subject?: string
  message: string
  author: string
  /** Author time, unix seconds */
  time: number
}

export function getMergeBase(analysis: ConflictAnalysis): MergeBase {
  return {
    hash: analysis.mergeBaseHash,
    subject: analysis.mergeBaseSubject,
    message: analysis.mergeBaseMessage,
    author: analysis.mergeBaseAuthor,
    time: analysis.mergeBaseTime,
  }
}

/**
 * The title of a merge conflict wherever the conflict is shown (the expanded row, the sub-windows, the preview):
 * "“Add login form” does not apply cleanly onto master". The grouping prefix of the subject is left out.
 */
export function formatConflictTitle(message: string, onto: string | null | undefined): string {
  const { autosquash, rest } = parseSubject(getSubjectLine(message))
  return `“${autosquash}${rest}” does not apply cleanly onto ${onto || "the baseline"}`
}

/** A file of the "Conflicts only" diff view */
export interface ConflictFileDiff {
  diff: FileDiff
  /** The file has no hunk with a conflict marker (a new or deleted file), so the whole diff is shown */
  isWholeFile: boolean
}

/**
 * Keeps only the hunks with conflict markers. A file without such a hunk keeps its whole diff, so every conflicting file
 * stays in the list: a modify/delete conflict has no markers at all.
 */
export function filterConflictHunks(fileDiffs: readonly FileDiff[]): ConflictFileDiff[] {
  return fileDiffs.map(diff => {
    const hunks = diff.hunks.filter(hasConflictMarkers)
    return hunks.length === 0 ? { diff, isWholeFile: true } : { diff: { ...diff, hunks }, isWholeFile: false }
  })
}

function hasConflictMarkers(hunk: string): boolean {
  return hunk.includes("<<<<<<<") || hunk.includes("|||||||") || hunk.includes("=======") || hunk.includes(">>>>>>>")
}
