import { runAppCommand } from "@/lib/app-commands"
import type { GitHubRepository } from "@/lib/pull-request"
import { repositoryName } from "@/lib/repository-config"
import type { ActivityEntry, ActivityKind, UndoState } from "@/stores/activity"

/*
 * The Activity window (spec/repository/activity.spec.md): the log of the selected repository in a window of its own,
 * opened by the main window only. The window reads the log itself, as the main window does; the main window tells it
 * over the link of the window (`@/lib/activity-window-link`) what only it knows (the shown repository, its branches,
 * the landed report, the Undo of this session's rewrites) and runs what it asks for. Every entry point asks the main
 * window (`openActivity`).
 */

export const ACTIVITY_WINDOW = "activity"

/** The chips of the window: the kinds of entries, each chip some of them */
export type ActivityFilter = "robots" | "pushes" | "updates" | "reviews" | "rewrites" | "errors"

export const ACTIVITY_FILTERS: readonly ActivityFilter[] = ["robots", "pushes", "updates", "reviews", "rewrites", "errors"]

export const ACTIVITY_FILTER_LABELS: Readonly<Record<ActivityFilter, string>> = {
  robots: "Robots",
  pushes: "Pushes",
  updates: "Updates",
  reviews: "Reviews",
  rewrites: "Rewrites",
  errors: "Errors",
}

const FILTER_OF_KIND: Readonly<Record<ActivityKind, ActivityFilter>> = {
  patronus: "robots",
  push: "pushes",
  update: "updates",
  landed: "updates",
  review: "reviews",
  rewrite: "rewrites",
  error: "errors",
}

/** The chip an entry is under */
export function activityFilterOf(entry: Pick<ActivityEntry, "kind">): ActivityFilter {
  return FILTER_OF_KIND[entry.kind]
}

/** What the log shows: the entries of a chip (`filter`, null: all) that have the text `query`, the agents' too while `agents` is on */
export interface ActivityQuery {
  readonly filter: ActivityFilter | null
  readonly query: string
  /** The Agents chip: the operations of the agents (their worktree a slot of the lease pool) show only while it is on */
  readonly agents: boolean
}

/** The entries `query` keeps: of its chip, whose title or description has its text, ignoring case, the agents' only while they show */
export function filterActivity(entries: readonly ActivityEntry[], { filter, query, agents }: ActivityQuery): ActivityEntry[] {
  return entries.filter(entry => (agents || entry.agent === undefined) && (filter === null || activityFilterOf(entry) === filter) && matchesQuery(entry, query))
}

/**
 * The counts of the chips: the entries of each kind and of all that have the text, the agents' only while they show; of
 * Agents, the agents' entries the chosen chip and the text keep, what its press adds or takes away
 */
export function countActivityFilters(entries: readonly ActivityEntry[], { filter, query, agents }: ActivityQuery): Record<ActivityFilter | "all" | "agents", number> {
  const counts = { all: 0, robots: 0, pushes: 0, updates: 0, reviews: 0, rewrites: 0, errors: 0, agents: 0 }
  for (const entry of entries) {
    if (!matchesQuery(entry, query)) {
      continue
    }
    const kind = activityFilterOf(entry)
    if (entry.agent !== undefined && (filter === null || kind === filter)) {
      counts.agents++
    }
    if (agents || entry.agent === undefined) {
      counts.all++
      counts[kind]++
    }
  }
  return counts
}

function matchesQuery(entry: ActivityEntry, query: string): boolean {
  const needle = query.trim().toLowerCase()
  return needle === "" || entry.title.toLowerCase().includes(needle) || (entry.description?.toLowerCase().includes(needle) ?? false)
}

/**
 * What Copy Diagnostics of an entry copies: what it says, when, and the operation it is of as the lines of the log name
 * it (`operation_id=…`), with the worktree of an agent's; null for an entry of no operation
 */
export function activityDiagnostics(entry: ActivityEntry): string | null {
  if (entry.operationId === undefined) {
    return null
  }
  const lines = [entry.title, ...(entry.description === undefined ? [] : [entry.description]), `${new Date(entry.time).toISOString()} operation_id=${entry.operationId}`]
  if (entry.agent !== undefined) {
    lines.push(`agent worktree=${entry.agent.worktree}`)
  }
  return lines.join("\n")
}

/** "Activity — branch-deck": the window names its repository */
export function activityWindowTitle(repositoryPath: string): string {
  return `Activity — ${repositoryName(repositoryPath)}`
}

/** What an open of the window shows (`kind` of `__INIT_DATA__`): the entries of a chip; none shows all */
export interface ActivityTarget {
  readonly kind?: ActivityFilter
}

/**
 * Opens Activity at `target`, from any window: the main window toggles its side panel, or opens the window where the
 * panel has no room (`useActivityPanel`); a target with a chip opens the panel at it
 */
export function openActivity(target: ActivityTarget = {}): void {
  runAppCommand("open_activity", target)
}

/** The links of a branch of a landed report, from the stores of the main window: the window shows them as its island's chips */
export interface LandedBranchLinks {
  readonly pullRequest?: { readonly number: number; readonly url: string }
  readonly robot?: { readonly url: string; readonly mode: string }
}

/** What the main window tells the window of its selected repository: what only it knows; the window reads the log itself */
export interface ActivityWindowState {
  readonly repositoryPath: string
  /** The name of the repository, in the title of the window */
  readonly repositoryName: string
  /** The GitHub repository of the repository, where the pull requests of the entries link; null while GitHub lists none */
  readonly gitHubRepository: GitHubRepository | null
  /** The Undo of the rewrites of this session, by entry id: only the main window can undo them (`canUndoActivity`) */
  readonly undo: Readonly<Record<string, UndoState>>
  /** The branches of the table: a word of an entry that names one shows its row */
  readonly branches: readonly string[]
  /** The commits of the undismissed landed report: an entry whose commits are among them offers Dismiss */
  readonly undismissed: readonly string[]
  /** The links of the branches of the landed entries, by name */
  readonly links: Readonly<Record<string, LandedBranchLinks>>
}

/** What `__INIT_DATA__` of the window carries */
export interface ActivityWindowData {
  readonly state: ActivityWindowState
  readonly kind: ActivityFilter | null
  /** When the window was opened: each open starts over with its chip and an empty search */
  readonly openedAt: number
}

/** What the window asks the main window for */
export type ActivityWindowRequest = { readonly id: string; readonly repositoryPath: string } & (
  | { readonly reveal: string }
  | { readonly undo: string }
  | { readonly dismiss: true }
  | { readonly clear: true }
)

/** The error of a request the main window refused, shown in the window */
export interface ActivityWindowFeedback {
  readonly title: string
  readonly message: string
}
