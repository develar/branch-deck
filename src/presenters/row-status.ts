// The page's words of the Status cell of a branch row as Rust decides it (`RowStatus` of the view's `nextSteps`,
// `domain_policy::row_status`; spec/branch/branch-row.spec.md, "Status cell"): what it says first, and the exceptional
// facts that stay beside it as a mark whose hover names them, so a running robot never hides a block or a conflict, nor
// they the robot. It decides nothing: a sync running lays over the cell as it passes, and a cell without a fact says
// the push state of the branch or what its sync did.
import type { PatronusRobot, RowFact, RowStatus } from "@/lib/bindings"
import { landedBranchDescription } from "@/lib/landed"
import type { ParkedState } from "@/lib/parked"
import { formatParkedLabel } from "@/lib/parked"
import { formatPushMode } from "@/lib/patronus"
import { getPushColor } from "@/lib/push-summary"
import { buildRemoteStatusTexts } from "@/lib/remote-status"
import type { StatusTexts } from "@/lib/remote-status"
import { stackProblemOf } from "@/lib/stack"
import type { Branch } from "@/stores/branch-sync"

/** What the first column of the cell shows: the robot of the row, its parked state, or a status text */
export type RowCell = { readonly kind: "robot"; readonly running: boolean } | { readonly kind: "parked" } | { readonly kind: "text"; readonly texts: StatusTexts }

/** The mark beside the first column: the colour of its strongest fact, and a line of each fact on its hover */
export interface RowMark {
  readonly color: "error" | "warning" | "success"
  readonly lines: readonly string[]
}

/** What the cell words beside the view: the branch, the baseline that landed it (`useLandedIn`), its robot, parked */
export interface RowContext {
  readonly branch: Branch
  readonly landedIn: string | null
  readonly robot: PatronusRobot | undefined
  readonly parked: ParkedState | null
}

/** The first column of the Status cell of a branch row */
export function presentRowCell(status: RowStatus | undefined, context: RowContext): RowCell {
  const { branch } = context
  if (branch.status === "Syncing") {
    return { kind: "text", texts: syncTexts(branch) }
  }
  const main = status?.main ?? null
  if (main?.kind === "robot" && context.robot !== undefined) {
    return { kind: "robot", running: main.running }
  }
  if (main?.kind === "parked" && context.parked !== null) {
    return { kind: "parked" }
  }
  return { kind: "text", texts: main === null ? pushTexts(branch) : factTexts(main, context) }
}

/** The mark of the facts beside the first column; null without one, and while a sync lays over the cell */
export function presentRowMark(status: RowStatus | undefined, context: RowContext): RowMark | null {
  const also = status?.also ?? []
  if (also.length === 0 || context.branch.status === "Syncing") {
    return null
  }
  const colors = also.map(markColor)
  const color = colors.includes("error") ? "error" : colors.includes("warning") ? "warning" : "success"
  return { color, lines: also.map(fact => factTexts(fact, context).tooltip) }
}

/**
 * The status of the branch preview, which shows its robot apart: a sync running, the first fact of the cell that is not
 * the robot, else the push state. The preview says "up to date" too: it is never quiet
 */
export function presentPreviewStatus(status: RowStatus | undefined, context: RowContext): StatusTexts {
  const { branch } = context
  if (branch.status === "Syncing") {
    return syncTexts(branch)
  }
  const facts = status === undefined ? [] : [...(status.main === null ? [] : [status.main]), ...status.also]
  const shown = facts.find(fact => fact.kind !== "robot")
  const texts = shown === undefined ? pushTexts(branch) : factTexts(shown, context)
  return { ...texts, quiet: undefined }
}

/** "syncing 3/10…": the sync builds the branch anew */
function syncTexts(branch: Branch): StatusTexts {
  const progress = `${branch.processedCount}/${branch.commitCount}`
  return { text: `syncing ${progress}…`, tooltip: `Syncing ${progress}…`, color: "neutral" }
}

/**
 * The push state, in the colour of the push button; up to date needs no action: quiet. Without a remote status, what
 * the sync did
 */
function pushTexts(branch: Branch): StatusTexts {
  if (branch.remoteStatus != null) {
    const texts = { ...buildRemoteStatusTexts(branch.remoteStatus, branch.stack?.effectiveBase ?? null), color: getPushColor(branch) ?? "neutral" }
    return branch.remoteStatus.action.kind === "upToDate" ? { ...texts, quiet: true } : texts
  }
  switch (branch.status) {
    case "Created":
      return { text: "created", tooltip: "Branch created", color: "success" }
    case "Updated":
      return { text: "updated", tooltip: "Branch updated", color: "primary" }
    default:
      return { text: "unchanged", tooltip: "No changes", color: "neutral" }
  }
}

/** The words of a fact of the cell */
function factTexts(fact: RowFact, { branch, landedIn, robot, parked }: RowContext): StatusTexts {
  switch (fact.kind) {
    case "syncFailed": {
      // a stack problem: its label, its message on hover
      if (fact.status === "AnalyzingConflict") {
        return { text: branch.statusText, tooltip: branch.statusText || "Analyzing conflict…", color: "warning", badge: true }
      }
      const problem = fact.status === "Error" ? stackProblemOf(branch) : null
      return { text: branch.statusText, tooltip: problem?.message ?? (branch.statusText || "Error"), color: "error", badge: true }
    }
    // not an error of its own, so muted; its ref is left as it was
    case "blocked":
      return {
        text: fact.by === null ? "blocked" : `blocked by ${fact.by}`,
        tooltip: branch.statusText || (fact.by === null ? "Blocked by its parent branch" : `Blocked by ${fact.by}`),
        color: "warning",
      }
    // the update archives it: its push state no longer matters; `info` is the merged hue
    case "landed":
      return landedIn === null ? { text: "landed", tooltip: "Already in master", color: "info" } : { text: "landed", tooltip: landedBranchDescription(landedIn), color: "info" }
    case "parked":
      return { text: "parked", tooltip: parked === null ? "Parked: kept local, never pushed" : formatParkedLabel(parked), color: "neutral" }
    case "robot": {
      const mode = robot === undefined ? "Safe Push" : formatPushMode(robot.pushMode)
      const state = fact.running ? "runs" : fact.failed ? "failed" : "finished"
      return { text: `${mode} ${state}`, tooltip: `${mode} ${state}`, color: fact.failed ? "error" : "primary" }
    }
  }
}

function markColor(fact: RowFact): RowMark["color"] {
  switch (fact.kind) {
    case "syncFailed":
      return "error"
    case "robot":
      return fact.failed ? "error" : "success"
    case "blocked":
    case "parked":
      return "warning"
    case "landed":
      return "success"
  }
}
