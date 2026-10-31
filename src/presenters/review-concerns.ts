import type { ConcernEvidence, ConcernName, EvidenceKind } from "@/lib/bindings"

/*
 * The words of the concerns of a review (spec/branch/review-concerns.spec.md, "In the Review Window"): Rust joins the
 * units into the tasks of the range, names each by its evidence and says which concern holds the main row of a file
 * of several (`Concern.elsewhere`); this words them for the tree and the headings of the story.
 */

/** What no evidence joined, as a group of the tree and the heading of its part of the story */
export const OTHER_CHANGES = "Other changes"

/** "proxy", "grep → accio", "open", "docs/guide", "Root", "Other changes" */
export function formatConcernName(name: ConcernName): string {
  switch (name.source) {
    case "token":
      return name.token
    case "replacement":
      return `${name.removed} → ${name.added}`
    case "declaration":
      return name.name
    case "directory":
      return name.directory === "" ? "Root" : name.directory
    case "rest":
      return OTHER_CHANGES
  }
}

/** "also in proxy": a row of a file in a concern that is not the one of its main row */
export function formatAlsoIn(name: ConcernName): string {
  return `also in ${formatConcernName(name)}`
}

/** "Its own row is under grep → accio": where the main row of a file of several concerns is */
export function formatHome(name: ConcernName): string {
  return `Its own row is under ${formatConcernName(name)}`
}

const EVIDENCE_WORDS: Readonly<Record<EvidenceKind, string>> = {
  uses: "the uses of",
  group: "one group of files",
  token: "the identifier",
  rest: "the file or the subject of a change",
}

/**
 * Why the parts of a concern are together, in the hover of its row: "Joined by the identifiers ij-mcp-proxy, ij-proxy ·
 * the uses of open"; each kind once, in the order Rust ranked its evidence (the most joins first)
 */
export function formatConcernEvidence(evidence: readonly ConcernEvidence[]): string | null {
  const details = new Map<EvidenceKind, string[]>()
  for (const entry of evidence) {
    const names = details.get(entry.kind) ?? []
    if (entry.detail !== null && !names.includes(entry.detail)) {
      names.push(entry.detail)
    }
    details.set(entry.kind, names)
  }
  if (details.size === 0) {
    return null
  }
  const parts = [...details].map(([kind, names]) => {
    const words = kind === "token" && names.length > 1 ? "the identifiers" : EVIDENCE_WORDS[kind]
    return names.length === 0 ? words : `${words} ${names.join(", ")}`
  })
  return `Joined by ${parts.join(" · ")}`
}
