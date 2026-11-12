import type { Concern, ConcernChange, ConcernEvidence, ConcernName, EvidenceKind, FormChange } from "@/lib/bindings"
import { FORM_WORDS } from "@/presenters/review-plan"

/*
 * The words of the concerns of a review (openspec/specs/branch/review-concerns/spec.md, "In the Review Window"): Rust joins the
 * units into the tasks of the range, names each by its evidence and says which concern holds the main row of a file
 * of several (`Concern.elsewhere`); this words them for the tree and the headings of the story.
 */

/** What no evidence joined, as a group of the tree and the heading of its part of the story */
export const OTHER_CHANGES = "Other changes"

/** "proxy", "grep → accio", "open", "guide.md", "docs/guide", "Root", "Other changes" */
export function formatConcernName(name: ConcernName): string {
  switch (name.source) {
    case "token":
      return name.token
    case "replacement":
      return `${name.removed} → ${name.added}`
    case "declaration":
      return name.name
    case "file":
      return name.path.slice(name.path.lastIndexOf("/") + 1)
    case "directory":
      return name.directory === "" ? "Root" : name.directory
    case "rest":
      return OTHER_CHANGES
  }
}

/** The parts of a form a title names: one of these alone, else "signature" */
const TITLED_FORM: ReadonlySet<FormChange> = new Set(["parameters", "reach", "constructors"])

/**
 * The verb of the title of a concern, what Rust says it does to its name (`Concern.change`): "Remove", "Add", "Change",
 * "Change the parameters of"; none for a replacement, whose name says it ("grep → accio"), for a concern that does
 * several things, and for one of no name of its own (the root, Other changes)
 */
function concernVerb(name: ConcernName, change: ConcernChange): string | null {
  if (name.source === "rest" || (name.source === "directory" && name.directory === "")) {
    return null
  }
  switch (change.kind) {
    case "adds":
      return "Add"
    case "removes":
      return "Remove"
    case "body":
      return "Change"
    case "form": {
      const [only] = change.changes
      return only !== undefined && change.changes.length === 1 && TITLED_FORM.has(only) ? `Change the ${FORM_WORDS[only]} of` : "Change the signature of"
    }
    case "replaces":
    case "mixed":
      return null
  }
}

/**
 * The title of a concern: what it does to its name, then the name ("Add probe", "Change the parameters of open",
 * "grep → accio", "docs/guide"); a title with a verb is a sentence, set as text, a bare name is code (`isCodeName`)
 */
export function presentConcernTitle(concern: Pick<Concern, "name" | "change">): { readonly label: string; readonly sentence: boolean } {
  const name = formatConcernName(concern.name)
  const verb = concernVerb(concern.name, concern.change)
  return verb === null ? { label: name, sentence: false } : { label: `${verb} ${name}`, sentence: true }
}

/** "also in proxy": a row of a file in a chapter of the story (a concern, a step) that is not the one of its main row */
export function formatAlsoIn(chapter: string): string {
  return `also in ${chapter}`
}

/** "in step 2", "in proxy": after the name of a row "also in" another chapter, the chapter of its own row */
export function formatHomeTag(reference: string): string {
  return `in ${reference}`
}

/** "Its own row is under grep → accio": where the main row of a file of several chapters is */
export function formatHome(chapter: string): string {
  return `Its own row is under ${chapter}`
}

const EVIDENCE_WORDS: Readonly<Record<EvidenceKind, string>> = {
  uses: "the uses of",
  group: "one group of files",
  token: "the identifier",
  rest: "the file or the subject of a change",
}

/** A kind of the evidence of a concern in words and the names it joined by: "Joined by the identifiers", `hide`, `show` */
export interface EvidencePart {
  readonly words: string
  /** The identifiers, declarations or files, code */
  readonly names: readonly string[]
}

/**
 * Why the parts of a concern are together, by kind (`EvidencePart`), each once in the order Rust ranked its evidence:
 * the facts of the opening of a step set the names as code
 */
export function presentConcernEvidence(evidence: readonly ConcernEvidence[]): EvidencePart[] {
  const details = new Map<EvidenceKind, string[]>()
  for (const entry of evidence) {
    const names = details.get(entry.kind) ?? []
    if (entry.detail !== null && !names.includes(entry.detail)) {
      names.push(entry.detail)
    }
    details.set(entry.kind, names)
  }
  return [...details].map(([kind, names], index) => {
    const words = kind === "token" && names.length > 1 ? "the identifiers" : EVIDENCE_WORDS[kind]
    return { words: index === 0 ? `Joined by ${words}` : words, names }
  })
}

/**
 * Why the parts of a concern are together, in the hover of its row: "Joined by the identifiers ij-mcp-proxy, ij-proxy ·
 * the uses of open"; each kind once, in the order Rust ranked its evidence (the most joins first)
 */
export function formatConcernEvidence(evidence: readonly ConcernEvidence[]): string | null {
  const parts = presentConcernEvidence(evidence).map(({ words, names }) => (names.length === 0 ? words : `${words} ${names.join(", ")}`))
  return parts.length === 0 ? null : parts.join(" · ")
}
