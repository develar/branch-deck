import type { LandingNodeKind } from "@/lib/bindings"
import { MOVE_KINDS } from "@/lib/landing-overview"
import { matchesAllWords, searchWords } from "@/lib/text-search"
import type { Branch } from "@/stores/branch-sync"

/**
 * A state chip of the filter bar, where a branch is on its way into master as the node of its row says it (the
 * sections of the landing overview); `all` takes every state
 */
export type BranchFilterChip = "all" | "yourMove" | "landing" | "waiting" | "inMaster"

/** The filter of the branch table: the branches with every word of `text` in their texts, in the state of `chip` */
export interface BranchFilter {
  readonly text: string
  readonly chip: BranchFilterChip
}

export const NO_FILTER: BranchFilter = { text: "", chip: "all" }

/** The filter bar shows from this many branches on, or while a filter is active: a shorter table is read in one look */
export const FILTER_BAR_MIN_BRANCHES = 8

/** A word is typed or a state chosen: the table shows only some branches */
export function isFilterActive(filter: BranchFilter): boolean {
  return filter.chip !== "all" || filter.text.trim() !== ""
}

type SearchedBranch = Pick<Branch, "name" | "issueKey" | "summary" | "descriptionTitle" | "commits">

/**
 * Whether every word of `text` is in the name of the branch, its issue key, its summary, the title of its description
 * or the subject of one of its commits, ignoring case; any branch for a blank text
 */
export function matchesBranchText(branch: SearchedBranch, text: string): boolean {
  return matchesAllWords(
    [branch.name, branch.issueKey ?? "", branch.summary, branch.descriptionTitle ?? "", ...branch.commits.map(commit => commit.strippedSubject)],
    searchWords(text),
  )
}

/** The chip of a node: the filled ones are the user's move; a parked branch has its own card, no chip */
function chipOfNode(kind: LandingNodeKind): Exclude<BranchFilterChip, "all"> | null {
  switch (kind) {
    case "ready":
    case "yourStep":
    case "needsFix":
      return "yourMove"
    case "landing":
    case "waiting":
    case "inMaster":
      return kind
    case "parked":
      return null
  }
}

/** The branches of each state chip, of all rows of the table */
export interface BranchChipMatches {
  /** In table order */
  readonly all: readonly string[]
  /** The node is filled: the next step is the user's, a fix first (`needsFix`, `yourStep`, `ready`) */
  readonly yourMove: ReadonlySet<string>
  /** A Safe Push lands the branch: its own, or one of a branch built on it, of the whole stack */
  readonly landing: ReadonlySet<string>
  /** It waits for others: reviewers, its parent, a robot, the first answer of Patronus */
  readonly waiting: ReadonlySet<string>
  /** The baseline or a merge has it, until the update archives it */
  readonly inMaster: ReadonlySet<string>
}

/**
 * The branches of each state chip among `names` (the branch rows, in table order), by the node of each row
 * (`nodeOf`, `getLandingNodeNow`); a branch without a node is in All only
 */
export function collectChipMatches(names: readonly string[], nodeOf: (name: string) => LandingNodeKind | null): BranchChipMatches {
  const matches = { yourMove: new Set<string>(), landing: new Set<string>(), waiting: new Set<string>(), inMaster: new Set<string>() }
  const all: string[] = []
  for (const name of names) {
    const kind = nodeOf(name)
    all.push(name)
    const chip = kind === null ? null : chipOfNode(kind)
    if (chip !== null) {
      matches[chip].add(name)
    }
  }
  return { all, ...matches }
}

/**
 * The rows F2 steps through: the user's moves, a fix first, then a step of the user, then the ready ones, each in
 * table order (`MOVE_KINDS`)
 */
export function movesByUrgency(names: readonly string[], nodeOf: (name: string) => LandingNodeKind | null): string[] {
  return MOVE_KINDS.flatMap(kind => names.filter(name => nodeOf(name) === kind))
}

/** The count of a chip: every branch for `all` */
export function chipCount(matches: BranchChipMatches, chip: BranchFilterChip): number {
  return chip === "all" ? matches.all.length : matches[chip].size
}

/** The branches the filter matches, in table order; null for an inactive filter, which shows all */
export function filterBranches(matches: BranchChipMatches, branches: Readonly<Record<string, SearchedBranch>>, filter: BranchFilter): string[] | null {
  if (!isFilterActive(filter)) {
    return null
  }
  return matches.all.filter(name => {
    const branch = branches[name]
    return branch !== undefined && (filter.chip === "all" || matches[filter.chip].has(name)) && matchesBranchText(branch, filter.text)
  })
}

/**
 * The branches to show: the matches, and those shown since the filter last changed (`shown`), so a branch that stops
 * matching (started, its robot done) stays where the user is looking until the filter changes; the same set when no
 * match is new
 */
export function keepShown(shown: ReadonlySet<string> | null, matches: readonly string[]): ReadonlySet<string> {
  if (shown !== null && matches.every(name => shown.has(name))) {
    return shown
  }
  return new Set([...(shown ?? []), ...matches])
}

const chipEmptyLines: Readonly<Record<Exclude<BranchFilterChip, "all">, string>> = {
  yourMove: "Nothing is your move",
  landing: "Nothing is landing",
  waiting: "Nothing waits for others",
  inMaster: "Nothing is in master",
}

const chipNoMatchLines: Readonly<Record<BranchFilterChip, string>> = {
  all: "No branches match",
  yourMove: "No branches of your move match",
  landing: "No landing branches match",
  waiting: "No waiting branches match",
  inMaster: "No branches in master match",
}

/** The line of a table the filter left empty: `No branches match “foo”`, or "Nothing is your move" for a chip alone */
export function formatNoMatch(filter: BranchFilter): string {
  const text = filter.text.trim()
  if (text === "" && filter.chip !== "all") {
    return chipEmptyLines[filter.chip]
  }
  return `${chipNoMatchLines[filter.chip]} “${text}”`
}

/** The label of a chip, Title Case as a tab, in the words of the landing overview */
export const CHIP_LABELS: Readonly<Record<BranchFilterChip, string>> = {
  all: "All",
  yourMove: "Your Move",
  landing: "Landing",
  waiting: "Waiting",
  inMaster: "In Master",
}

/** The chips in the order of the bar: the user's moves, then the way into master */
export const CHIPS: readonly BranchFilterChip[] = ["all", "yourMove", "landing", "waiting", "inMaster"]
