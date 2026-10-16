import type { UpdateCheck, UpdateState } from "@/lib/bindings"
import { formatRelativeTime } from "@/lib/time"
import { transferAmount } from "@/lib/transfer-progress"
import type { TimedStep, UpdateRun, UpdateStep } from "@/lib/update-progress"
import { progressPercent, stepsAfter } from "@/lib/update-progress"

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

/** The first `NAMED_FILES` of `files`, then how many more of `count` (the backend reports the first files of all) */
function formatFileList(files: readonly string[], count: number): string {
  const named = files.slice(0, NAMED_FILES).join(", ")
  return count > NAMED_FILES ? `${named} and ${count - NAMED_FILES} more` : named
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
        summary: state.fileCount === 0 ? `Your commits conflict with ${upstream}` : `Conflicts in ${fileCount(state.fileCount)}: ${formatFileList(state.files, state.fileCount)}`,
        short: state.fileCount === 0 ? "conflicts" : `conflicts in ${fileCount(state.fileCount)}`,
      }
    case "blocked":
      return {
        tone: "warning",
        summary: `Your uncommitted changes in ${formatFileList(state.paths, state.pathCount)} overlap ${upstream}`,
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

/** The label of the button while a run is on: one word for the whole run, "Updating 62%"; "Updating…" before 1 % */
export function runLabel(run: UpdateRun, progress: number): string {
  const verb = run.kind === "fetch" ? "Fetching" : "Updating"
  const percent = progressPercent(progress)
  return percent === 0 ? `${verb}…` : `${verb} ${percent}%`
}

/** Where a step of the tooltip list is */
export type RunStepState = "done" | "running" | "next"

export interface RunStepLine {
  readonly step: UpdateStep
  readonly state: RunStepState
  readonly text: string
}

/** The words of the wait for the repository lock, as the error of a lock held for the whole wait (`LOCKED_MESSAGE`) */
const WAITING_SENTENCE = "Waiting for Branch Deck or bd to finish…"

/**
 * The sentence of the running step: the last progress line of git once it reports a stage of the fetch, else the
 * step. Never the state of the last check: before the first stage git connects and negotiates, which takes long on a
 * big repo.
 */
export function runningStepSentence(run: UpdateRun, upstream: string): string {
  switch (run.step) {
    case "fetch":
      return run.fetch?.line ?? `Fetching ${upstream}…`
    case "waiting":
      return WAITING_SENTENCE
    case "check":
      return `Checking the update onto ${upstream}…`
    case "apply":
      return `Updating to ${upstream}…`
    case "sync":
      return "Syncing branches…"
  }
}

function doneSentence(step: UpdateStep, detail: string | null, upstream: string): string {
  switch (step) {
    case "fetch":
      return detail === null ? `Fetched ${upstream}` : `Fetched ${upstream} · ${detail}`
    case "waiting":
      return "Waited for Branch Deck or bd"
    case "check":
      return `Checked the update onto ${upstream}`
    case "apply":
      return `Updated to ${upstream}`
    case "sync":
      return "Synced branches"
  }
}

function nextSentence(step: TimedStep, upstream: string): string {
  switch (step) {
    case "fetch":
      return `Fetch ${upstream}`
    case "check":
      return "Check the update"
    case "apply":
      return `Update to ${upstream}`
    case "sync":
      return "Sync branches"
  }
}

/**
 * The steps of the run for its tooltip: those done (a wait that ended is no step of its own), the running one, and
 * those the run still expects; a step the check made needless is not listed
 */
export function runStepLines(run: UpdateRun, upstream: string): RunStepLine[] {
  return [
    ...run.done.filter(it => it.step !== "waiting").map(it => ({ step: it.step, state: "done" as const, text: doneSentence(it.step, it.detail, upstream) })),
    { step: run.step, state: "running", text: runningStepSentence(run, upstream) },
    ...stepsAfter(run.plan, run.step).map(step => ({ step, state: "next" as const, text: nextSentence(step, upstream) })),
  ]
}

/**
 * The words after "Incoming from origin/master: " while a run is on: the amount and the rate while the fetch receives,
 * else the step in lower case, e.g. "checking the update…"; the percent is on the button
 */
export function runFooterWords(run: UpdateRun): string {
  switch (run.step) {
    case "fetch":
      return transferAmount(run.fetch) ?? "fetching…"
    case "waiting":
      return "waiting for Branch Deck or bd…"
    case "check":
      return "checking the update…"
    case "apply":
      return "updating the files…"
    case "sync":
      return "syncing branches…"
  }
}
