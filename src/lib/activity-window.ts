import { runAppCommand } from "@/lib/app-commands"
import { repositoryName } from "@/lib/repository-config"
import type { ActivityEntry, ActivityKind } from "@/stores/activity"

/*
 * The Activity window (spec/repository/activity.spec.md): the log of the selected repository in a window of its own,
 * opened by the main window only, which owns the log and sends it over the link of the window
 * (`@/lib/activity-window-link`). Every entry point asks the main window (`openActivity`).
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

/** The entries of the chip (`null`: all) whose title or description has `query`, ignoring case */
export function filterActivity(entries: readonly ActivityEntry[], filter: ActivityFilter | null, query: string): ActivityEntry[] {
  return entries.filter(entry => (filter === null || activityFilterOf(entry) === filter) && matchesQuery(entry, query))
}

/** The entries of each chip that have `query`, and of all: the counts of the chips */
export function countActivityFilters(entries: readonly ActivityEntry[], query: string): Record<ActivityFilter | "all", number> {
  const counts = { all: 0, robots: 0, pushes: 0, updates: 0, reviews: 0, rewrites: 0, errors: 0 }
  for (const entry of entries) {
    if (matchesQuery(entry, query)) {
      counts.all++
      counts[activityFilterOf(entry)]++
    }
  }
  return counts
}

function matchesQuery(entry: ActivityEntry, query: string): boolean {
  const needle = query.trim().toLowerCase()
  return needle === "" || entry.title.toLowerCase().includes(needle) || (entry.description?.toLowerCase().includes(needle) ?? false)
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

/** What the main window tells the window of the log of its selected repository */
export interface ActivityWindowState {
  readonly repositoryPath: string
  /** The name of the repository, in the title of the window */
  readonly repositoryName: string
  /** The log of the repository, newest first */
  readonly entries: readonly ActivityEntry[]
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
