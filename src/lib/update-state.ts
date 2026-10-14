import type { UpdateCheck, UpdateState } from "@/lib/bindings"

/** How the Update button shows the state: muted, a success check, or a warning */
export type UpdateTone = "muted" | "success" | "warning"

export interface UpdateStateDescription {
  tone: UpdateTone
  /** A sentence for the tooltip, e.g. "Rebases cleanly onto origin/master" */
  summary: string
  /** Lower-case words after the upstream name, e.g. "rebases cleanly" */
  short: string
}

/** At most this many file names in a sentence */
const NAMED_FILES = 3

function formatFileList(files: readonly string[]): string {
  const named = files.slice(0, NAMED_FILES).join(", ")
  return files.length > NAMED_FILES ? `${named} and ${files.length - NAMED_FILES} more` : named
}

function fileCount(count: number): string {
  return count === 1 ? "1 file" : `${count} files`
}

/** The words for a checked update. Deliberately without a count of upstream commits: only conflicts matter. */
export function describeUpdateState(state: UpdateState, upstream: string): UpdateStateDescription {
  switch (state.status) {
    case "upToDate":
      return { tone: "muted", summary: `Up to date with ${upstream}`, short: "up to date" }
    case "clean":
      return state.kind === "fastForward"
        ? { tone: "success", summary: `Fast-forwards to ${upstream}`, short: "fast-forwards" }
        : { tone: "success", summary: `Rebases cleanly onto ${upstream}`, short: "rebases cleanly" }
    case "conflict":
      return {
        tone: "warning",
        summary: state.files.length === 0
          ? `Your commits conflict with ${upstream}`
          : `Conflicts in ${fileCount(state.files.length)}: ${formatFileList(state.files)}`,
        short: state.files.length === 0 ? "conflicts" : `conflicts in ${fileCount(state.files.length)}`,
      }
    case "blocked":
      return {
        tone: "warning",
        summary: `Your uncommitted changes in ${formatFileList(state.paths)} overlap ${upstream}`,
        short: "blocked by your uncommitted changes",
      }
  }
}

/** The last fetch of the baseline (ms): Branch Deck's own, or one elsewhere (`FETCH_HEAD`), whichever is newer */
export function lastFetchTime(update: UpdateCheck | null, ownFetchTime: number | undefined): number | null {
  const elsewhere = update?.fetchedAt == null ? null : update.fetchedAt * 1000
  const own = ownFetchTime ?? null
  if (elsewhere === null || own === null) {
    return elsewhere ?? own
  }
  return Math.max(elsewhere, own)
}
