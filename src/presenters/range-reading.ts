import type { ClaimConflict, QuietProof, RangeReadingView, ReadingSummary, RequestReadingState, StackReading, StackReadingState, Unproven } from "@/lib/bindings"

/*
 * The words of what a range leaves to read (openspec/specs/review/range-reading/spec.md): the verdict Rust proved from
 * the marks of the diffs (`domain_policy::range_reading`), the author's word it proved wrong (`author_claim`), the sum
 * of the layers of a stack, and what a row of Your Turn says of its request read ahead. Words only: what is left, what
 * cannot be proven and what contradicts the author is decided in Rust.
 */

const PROOF_WORDS: Record<QuietProof, string> = {
  formatting: "formatting",
  comments: "comments",
  renames: "renamed names",
  imports: "imports",
  moves: "moves",
  renamedFiles: "moved files",
  lineEnds: "line endings",
}

const UNPROVEN_WORDS: Record<Unproven, string> = {
  binary: "binary",
  tooLarge: "too large",
  submodule: "a submodule",
  missing: "not in the clone",
  noGrammar: "no grammar",
  renamesPending: "renames being checked",
  renamesUnanswered: "renames the index could not prove",
}

/** "1 change", "3 changes" */
function changes(count: number): string {
  return `${count} ${count === 1 ? "change" : "changes"}`
}

/** "1 file", "3 files" */
function files(count: number): string {
  return `${count} ${count === 1 ? "file" : "files"}`
}

/** "3 changes to read in 2 files"; empty for none */
function left(toRead: number, fileCount: number): string {
  return toRead === 0 ? "" : `${changes(toRead)} to read in ${files(fileCount)}`
}

/**
 * The verdict of a reading on one line: "Nothing to read · formatting, renamed names", "Tests only · 3 changes to read
 * in 2 files", "Docs only · 1 change to read in 1 file", "12 changes to read in 5 files", with "· 2 files not proven"
 * and "· checking renames" after it where they hold
 */
export function formatReading(summary: ReadingSummary): string {
  const { verdict } = summary
  const parts: string[] = []
  switch (verdict.kind) {
    case "nothingToRead":
      parts.push(verdict.proofs.length === 0 ? "Nothing to read" : `Nothing to read · ${verdict.proofs.map(proof => PROOF_WORDS[proof]).join(", ")}`)
      break
    case "testsOnly":
      parts.push("Tests only", left(verdict.toRead, verdict.files))
      break
    case "docsOnly":
      parts.push("Docs only", left(verdict.toRead, verdict.files))
      break
    case "toRead":
      parts.push(left(verdict.toRead, verdict.files))
      break
  }
  if (summary.unproven > 0) {
    parts.push(`${files(summary.unproven)} not proven`)
  }
  if (summary.pending) {
    parts.push("checking renames")
  }
  const line = parts.filter(part => part !== "").join(" · ")
  return line.charAt(0).toUpperCase() + line.slice(1)
}

/** For a hover: the files that cannot be proven, each with why ("Not proven: icon.png (binary), build.sh (no grammar)"); null for none */
export function describeUnproven(view: RangeReadingView): string | null {
  const shown = view.reading.unproven.filter(file => file.why !== "renamesPending")
  return shown.length === 0 ? null : `Not proven: ${shown.map(file => `${file.path} (${UNPROVEN_WORDS[file.why]})`).join(", ")}`
}

const CLAIM_WORDS = { refactor: "refactoring", tests: "tests", docs: "docs" } as const
const OUTSIDE_WORDS = { refactor: "of code", tests: "outside the tests", docs: "outside the docs" } as const

/**
 * The label the diff proves wrong: the label chip itself says it, in the warning tone with `describeClaimConflict` as
 * its hover, so no other chip repeats it; null for a word of the title, or none
 */
export function claimedLabel(conflict: ClaimConflict | null): string | null {
  const source = conflict?.claim.source
  return source?.kind === "label" ? source.name : null
}

/**
 * The labels as their chips show them: the one the diff proves wrong (`claimed`) first, so a cut list never hides it
 */
export function orderLabels(labels: readonly string[], claimed: string | null): readonly string[] {
  return claimed === null || !labels.includes(claimed) ? labels : [claimed, ...labels.filter(label => label !== claimed)]
}

/**
 * The chip of a word of the title the diff proves wrong, compact, its count in its hover: "refactor: in the title";
 * null for a label, whose own chip says it (`claimedLabel`)
 */
export function formatClaimConflict(conflict: ClaimConflict): string | null {
  const { source } = conflict.claim
  return source.kind === "label" ? null : `${source.prefix}: in the title`
}

/** For the hover of the chip: what the author says, and what the diff leaves */
export function describeClaimConflict(conflict: ClaimConflict): string {
  const { kind } = conflict.claim
  const one = conflict.toRead === 1
  return `The author calls it ${CLAIM_WORDS[kind]}, but ${changes(conflict.toRead)} ${OUTSIDE_WORDS[kind]} in ${files(conflict.files)} ${one ? "is" : "are"} left to read: no formatting, rename, import or move proves ${one ? "it" : "them"}`
}

/**
 * The sum of the layers of a stack, each count of its own unit: "1 of 7 with nothing to read · 14 changes to read",
 * "Nothing to read" when no layer has anything, "2 being read" while some are; "3 of 7 to read" when the layers left
 * have no change to count (files not proven)
 */
export function formatStackReading(stack: StackReading): string {
  if (stack.unknown === 0 && stack.toReadLayers === 0) {
    return "Nothing to read"
  }
  const parts = [
    stack.nothingToRead > 0 ? `${stack.nothingToRead} of ${stack.layers} with nothing to read` : "",
    stack.toRead > 0 ? `${changes(stack.toRead)} to read` : stack.toReadLayers > 0 ? `${stack.toReadLayers} of ${stack.layers} to read` : "",
    stack.unknown > 0 ? `${stack.unknown} being read` : "",
  ]
  return parts.filter(part => part !== "").join(" · ")
}

/**
 * The verdict of a reading as a chip of a narrow list (the layers of a stack in its review window), its full words in
 * its hover (`formatReading`): "Nothing to read", "4 to read", "Tests only · 1 to read", "Docs only · 2 to read",
 * "1 not proven" when the files left have no change to count; "· checking" while a rename waits for the index
 */
export function formatCompactReading(summary: ReadingSummary): string {
  const { verdict } = summary
  if (verdict.kind === "nothingToRead") {
    return "Nothing to read"
  }
  const count = verdict.toRead > 0 ? `${verdict.toRead} to read` : summary.unproven > 0 ? `${summary.unproven} not proven` : ""
  const parts = [verdict.kind === "testsOnly" ? "Tests only" : verdict.kind === "docsOnly" ? "Docs only" : "", count, summary.pending ? "checking" : ""]
  const line = parts.filter(part => part !== "").join(" · ")
  return line.charAt(0).toUpperCase() + line.slice(1)
}

/** What is said while a request or a stack is read ahead */
export const CHECKING = "Checking…"

/** What a row of Your Turn says of what its request leaves to read: "Checking…" while it is read ahead, else its verdict */
export function formatRequestReading(state: RequestReadingState): string {
  return state.kind === "checking" ? CHECKING : formatReading(state.view.summary)
}

/** What the header of a stack of Your Turn says of its layers: "Checking…" while one is read ahead, else their sum */
export function formatStackRowReading(state: StackReadingState): string {
  return state.kind === "checking" ? CHECKING : formatStackReading(state.summary)
}
