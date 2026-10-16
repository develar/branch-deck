import type { TransferProgress, UpdateCheck, UpdatePhase, UpdateState } from "@/lib/bindings"
import { formatRelativeTime } from "@/lib/time"
import { transferLabel } from "@/lib/transfer-progress"

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

export function fileCount(count: number): string {
  return count === 1 ? "1 file" : `${count} files`
}

/** The words for a checked update. Deliberately without a count of upstream commits: only conflicts matter. */
export function describeUpdateState(state: UpdateState, upstream: string): UpdateStateDescription {
  switch (state.status) {
    case "upToDate":
      return { tone: "muted", summary: `Up to date with ${upstream}`, short: "nothing new" }
    case "clean":
      return state.kind === "fastForward"
        ? { tone: "success", summary: `Fast-forwards to ${upstream}`, short: "fast-forwards" }
        : { tone: "success", summary: `Rebases cleanly onto ${upstream}`, short: "rebases cleanly" }
    case "conflict":
      return {
        tone: "warning",
        summary: state.files.length === 0 ? `Your commits conflict with ${upstream}` : `Conflicts in ${fileCount(state.files.length)}: ${formatFileList(state.files)}`,
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

/**
 * The state as of the last fetch (`fetchTime`, null before the first one). Before it, "Up to date with origin/master"
 * or "Rebases cleanly" would claim what nobody asked the remote: muted, "origin/master not fetched yet". A warning,
 * found against the copy of the clone, keeps its words and its icon, then says that nothing was fetched.
 */
export function describeAsFetched(description: UpdateStateDescription, upstream: string, fetchTime: number | null): UpdateStateDescription {
  if (fetchTime !== null) {
    return description
  }
  if (description.tone === "warning") {
    return { tone: "warning", summary: `${description.summary} · ${upstream} not fetched yet`, short: `${description.short} · not fetched yet` }
  }
  return { tone: "muted", summary: `${upstream} not fetched yet`, short: "not fetched yet" }
}

/** The second line of the tooltip of the Update button, before the shortcut: "Fetched 5 minutes ago · fetch and update" */
export function fetchAndUpdateLine(fetchTime: number | null, now: number = Date.now()): string {
  return fetchTime === null ? "fetch and update" : `Fetched ${formatRelativeTime(fetchTime, now)} · fetch and update`
}

/**
 * The words after "Incoming from origin/master: " in the footer of the outgoing overview, of a state `describeAsFetched`
 * gave: "rebases cleanly · your wsm landed · fetched 5 minutes ago", "not fetched yet"
 */
export function updateFooterWords(description: UpdateStateDescription, fetchTime: number | null, landing: string | null, now: number = Date.now()): string {
  const fetched = fetchTime === null ? null : `fetched ${formatRelativeTime(fetchTime, now)}`
  return [description.short, landing, fetched].filter(part => part !== null).join(" · ")
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

const PHASE_LABELS: Record<UpdatePhase, string> = {
  fetching: "Fetching…",
  checking: "Checking…",
  updating: "Updating…",
}

/** The label of a running update, e.g. "Fetching 45%"; "Fetching…" until git reports a stage */
export function updatePhaseLabel(phase: UpdatePhase, fetch: TransferProgress | null): string {
  return phase === "fetching" ? transferLabel("Fetching", fetch) : PHASE_LABELS[phase]
}

/**
 * The tooltip sentence of a running update: the last progress line of git once it reports a stage, else the step.
 * Never the state of the last check: before the first stage git connects and negotiates, which takes long on a big repo.
 */
export function runningUpdateSummary(phase: UpdatePhase, fetch: TransferProgress | null, upstream: string): string {
  switch (phase) {
    case "fetching":
      return fetch?.line ?? `Fetching ${upstream}…`
    case "checking":
      return `Checking the update onto ${upstream}…`
    case "updating":
      return `Updating to ${upstream}…`
  }
}
