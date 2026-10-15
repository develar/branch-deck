import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { findBranchRobot, isRobotActive, needsRobotAttention } from "@/lib/patronus"
import { formatAuthorAttention } from "@/lib/pull-request"
import { summarizePushState } from "@/lib/push-summary"
import type { Branch } from "@/stores/branch-sync"

/** A state chip of the filter bar; `all` takes every state */
export type BranchFilterChip = "all" | "toPush" | "running" | "attention"

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
  const words = text.toLowerCase().split(/\s+/).filter(word => word !== "")
  if (words.length === 0) {
    return true
  }
  const texts = [branch.name, branch.issueKey ?? "", branch.summary, branch.descriptionTitle ?? "", ...branch.commits.map(commit => commit.strippedSubject)]
    .map(it => it.toLowerCase())
  return words.every(word => texts.some(it => it.includes(word)))
}

/**
 * A branch that waits for the user: an error or a conflict, blocked by its stack parent, a failed robot of the branch as
 * it is (also one that failed for missing approvals), or an open pull request with changes requested or failing checks
 * (`needsRobotAttention`, the robots indicator's, and `formatAuthorAttention`, the outgoing attention)
 */
export function needsAttention(branch: Branch, robot: PatronusRobot | undefined, pullRequest: PullRequest | undefined): boolean {
  if (branch.hasError || branch.status === "Error" || branch.status === "MergeConflict" || branch.status === "Blocked") {
    return true
  }
  return (robot !== undefined && needsRobotAttention(robot, branch.commits[0]?.hash, branch.remoteStatus))
    || (pullRequest !== undefined && formatAuthorAttention(pullRequest) !== null)
}

/** What the state chips look at, from the stores */
export interface BranchStateSources {
  readonly branches: Readonly<Record<string, Branch>>
  readonly robots: Readonly<Record<string, PatronusRobot>> | undefined
  readonly pullRequests: Readonly<Record<string, PullRequest>>
  /** The fetched baseline has every commit of the branch: nothing is left to push (`useIsFullyLanded`) */
  readonly isLanded: (name: string) => boolean
}

/** The branches of each state chip, of all rows of the table */
export interface BranchChipMatches {
  /** In table order */
  readonly all: readonly string[]
  /** A push has work for the remote, by the rule of the outgoing counts (`summarizePushState`) */
  readonly toPush: ReadonlySet<string>
  /** A robot of the branch runs or cancels */
  readonly running: ReadonlySet<string>
  readonly attention: ReadonlySet<string>
}

/** The branches of each state chip among `names` (the branch rows, in table order) */
export function collectChipMatches(names: readonly string[], sources: BranchStateSources): BranchChipMatches {
  const branches = names.flatMap(name => sources.branches[name] ?? [])
  const toPush = new Set(summarizePushState(branches, sources.isLanded).branches.map(it => it.name))
  const running = new Set<string>()
  const attention = new Set<string>()
  for (const branch of branches) {
    const robot = findBranchRobot(sources.robots, branch.name, branch.commits[0]?.hash, branch.remoteStatus?.remoteTip ?? null)
    if (robot !== undefined && isRobotActive(robot)) {
      running.add(branch.name)
    }
    if (needsAttention(branch, robot, sources.pullRequests[branch.name])) {
      attention.add(branch.name)
    }
  }
  return { all: branches.map(branch => branch.name), toPush, running, attention }
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
  return matches.all.filter((name) => {
    const branch = branches[name]
    return branch !== undefined && (filter.chip === "all" || matches[filter.chip].has(name)) && matchesBranchText(branch, filter.text)
  })
}

/**
 * The branches to show: the matches, and those shown since the filter last changed (`shown`), so a branch that stops
 * matching (pushed, its robot done) stays where the user is looking until the filter changes; the same set when no
 * match is new
 */
export function keepShown(shown: ReadonlySet<string> | null, matches: readonly string[]): ReadonlySet<string> {
  if (shown !== null && matches.every(name => shown.has(name))) {
    return shown
  }
  return new Set([...shown ?? [], ...matches])
}

const chipEmptyLines: Readonly<Record<Exclude<BranchFilterChip, "all">, string>> = {
  toPush: "Nothing to push",
  running: "No robot is running",
  attention: "Nothing needs attention",
}

const chipNoMatchLines: Readonly<Record<BranchFilterChip, string>> = {
  all: "No branches match",
  toPush: "No branches to push match",
  running: "No branches with a running robot match",
  attention: "No branches that need attention match",
}

/** The line of a table the filter left empty: `No branches match “foo”`, or "Nothing to push" for a chip alone */
export function formatNoMatch(filter: BranchFilter): string {
  const text = filter.text.trim()
  if (text === "" && filter.chip !== "all") {
    return chipEmptyLines[filter.chip]
  }
  return `${chipNoMatchLines[filter.chip]} “${text}”`
}

/** The label of a chip, Title Case as a tab */
export const CHIP_LABELS: Readonly<Record<BranchFilterChip, string>> = {
  all: "All",
  toPush: "To Push",
  running: "Running",
  attention: "Attention",
}

/** The chips in the order of the bar */
export const CHIPS: readonly BranchFilterChip[] = ["all", "toPush", "running", "attention"]
