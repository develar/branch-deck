import type { ClaimConflict, QuietProof, RangeReadingView, ReadingSummary, StackReading, Unproven } from "@/lib/bindings"

/*
 * The words of what a range leaves to read (openspec/specs/review/range-reading/spec.md): the verdict Rust proved from
 * the marks of the diffs (`domain_policy::range_reading`), the author's word it proved wrong (`author_claim`), the sum
 * of the layers of a stack. Words only: what is left, what cannot be proven and what contradicts the author is decided
 * in Rust.
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
 * The chip of an author's word the diff proves wrong: "Labelled refactoring · 3 changes to read", "refactor: in the
 * title · 3 changes to read"
 */
export function formatClaimConflict(conflict: ClaimConflict): string {
  const { source } = conflict.claim
  const said = source.kind === "label" ? `Labelled ${source.name}` : `${source.prefix}: in the title`
  return `${said} · ${changes(conflict.toRead)} to read`
}

/** For the hover of the chip: what the author says, and what the diff leaves */
export function describeClaimConflict(conflict: ClaimConflict): string {
  const { kind } = conflict.claim
  const one = conflict.toRead === 1
  return `The author calls it ${CLAIM_WORDS[kind]}, but ${changes(conflict.toRead)} ${OUTSIDE_WORDS[kind]} in ${files(conflict.files)} ${one ? "is" : "are"} left to read: no formatting, rename, import or move proves ${one ? "it" : "them"}`
}

/** The sum of the layers of a stack: "10 with nothing to read · 2 to read · 7 changes", "1 being read" while one is */
export function formatStackReading(stack: StackReading): string {
  const parts = [
    stack.nothingToRead > 0 ? `${stack.nothingToRead} with nothing to read` : "",
    stack.toReadLayers > 0 ? `${stack.toReadLayers} to read` : "",
    stack.toRead > 0 ? changes(stack.toRead) : "",
    stack.unknown > 0 ? `${stack.unknown} being read` : "",
  ]
  return parts.filter(part => part !== "").join(" · ")
}
