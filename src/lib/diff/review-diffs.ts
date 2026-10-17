import type { CommitFile, CommitPatch } from "@/lib/bindings"
import { getCommitFileDiff } from "@/lib/diff/commit-patch"
import type { CommitFileDiff } from "@/lib/diff/commit-patch"
import { getDiffWorkerPool } from "@/lib/diff/diff-engine"
import { markSubWindowOpenPhase } from "@/lib/sub-window"

// What a review shows of each file at first (`CommitReviewCode`), and the first screen readied before its diffs mount

/** A file with more changed lines starts collapsed: its header is enough to know it is there */
export const REVIEW_COLLAPSE_CHANGED_LINES = 1000

/** Lock files and generated files start collapsed */
const GENERATED_FILE = /(^|\/)(pnpm-lock\.yaml|package-lock\.json|yarn\.lock|Cargo\.lock|Gemfile\.lock|poetry\.lock|go\.sum)$|\.(min\.js|min\.css|snap|map)$/

const lineCountFormat = new Intl.NumberFormat("en")

/** The files a review shows first, from the initial one, the collapsed ones skipped: about a screen */
export const FIRST_SCREEN_FILES = 6
/** How long the engine may take at all: without it no line shows, not even plain text */
const ENGINE_BUDGET_MS = 1000
/** How long the first files may take to highlight: past it they show plain and are highlighted on screen */
const PRIME_BUDGET_MS = 80

/** Why a file shows no lines, under its header */
export function getNotice(entry: CommitFileDiff): string | null {
  if (entry.file.binary) {
    return "Binary file"
  }
  if (entry.omittedLines !== null) {
    return `Too large to show: ${lineCountFormat.format(entry.omittedLines)} changed lines`
  }
  if (entry.fileDiff === undefined || entry.fileDiff.hunks.length === 0) {
    return "No changed lines"
  }
  return null
}

/** Lock and generated files, files over `REVIEW_COLLAPSE_CHANGED_LINES` and files without lines start collapsed */
export function startsCollapsed(entry: CommitFileDiff): boolean {
  return getNotice(entry) !== null || entry.file.additions + entry.file.deletions > REVIEW_COLLAPSE_CHANGED_LINES || GENERATED_FILE.test(entry.file.path)
}

function withBudget(promise: Promise<unknown>, ms: number): Promise<unknown> {
  return Promise.race([promise.catch(() => undefined), new Promise(resolve => setTimeout(resolve, ms))])
}

/**
 * Readies the first screen of a review before its diffs mount: the engine initialised (within `ENGINE_BUDGET_MS`) and
 * the files from `initialPath` highlighted into the pool's cache (within `PRIME_BUDGET_MS`), so `CodeView` renders them
 * highlighted in its first render: Pierre renders on animation frames after that, and a hidden window runs none. Never
 * fails: past a budget the diffs mount with what is there.
 */
export async function prepareFirstDiffs(patch: CommitPatch, files: readonly CommitFile[], initialPath: string | null): Promise<void> {
  const pool = getDiffWorkerPool()
  await withBudget(pool.initialize(), ENGINE_BUDGET_MS)
  markSubWindowOpenPhase("engine")
  if (!pool.isWorkingPool()) {
    return
  }
  const start =
    initialPath === null
      ? 0
      : Math.max(
          0,
          files.findIndex(file => file.path === initialPath),
        )
  const first: Promise<void>[] = []
  for (const file of files.slice(start)) {
    const entry = getCommitFileDiff(patch, file)
    if (entry.fileDiff !== undefined && !startsCollapsed(entry)) {
      first.push(pool.primeDiffHighlightCache(entry.fileDiff))
      if (first.length === FIRST_SCREEN_FILES) {
        break
      }
    }
  }
  await withBudget(Promise.all(first), PRIME_BUDGET_MS)
  markSubWindowOpenPhase("primed")
}
