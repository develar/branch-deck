import { useRef, useState } from "react"
import type { ReactNode, RefObject } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { ActivityLog } from "@/components/repository/ActivityLog"
import type { ActivityLogActions } from "@/components/repository/ActivityLog"
import { FilterChip, FilterChips } from "@/components/shared/FilterChips"
import { SearchField } from "@/components/shared/SearchField"
import { ACTIVITY_FILTER_LABELS, ACTIVITY_FILTERS, countActivityFilters, filterActivity } from "@/lib/activity-window"
import type { ActivityFilter } from "@/lib/activity-window"
import type { ActivityEntry } from "@/stores/activity"

/** The value of the All chip: every kind */
const ALL = "all"

export interface ActivityPanelContentProps {
  /** The log of the repository, newest first */
  entries: readonly ActivityEntry[]
  /** The chip an open names ("Show in Activity": Robots), null for All; each open mounts it anew (a `key`) */
  initialFilter: ActivityFilter | null
  /** When it was opened: dates the days */
  openedAt: number
  actions: ActivityLogActions
  /**
   * `window`: the search and the chips on the canvas, the log in its own island; `panel`: both inside the island of the
   * side panel, under its header
   */
  layout: "window" | "panel"
  /** The log, which the search leaves to and which a keyboard open of the panel focuses */
  listRef?: RefObject<HTMLDivElement | null>
  /** `/` focuses the search: in the window; the side panel leaves `/` to the branch table */
  slashFocus?: boolean
}

/**
 * Activity of a repository (spec/repository/activity.spec.md): the search and the chips of the kinds, then the log, in
 * the Activity window and in the side panel of the main window alike. Only the props: the window gets the log over its
 * link, the panel from the stores of the main window.
 */
export function ActivityPanelContent({ entries, initialFilter, openedAt, actions, layout, listRef, slashFocus = true }: ActivityPanelContentProps) {
  const [filter, setFilter] = useState<ActivityFilter | null>(initialFilter)
  const [query, setQuery] = useState("")
  const ownListRef = useRef<HTMLDivElement>(null)
  const list = listRef ?? ownListRef
  const counts = countActivityFilters(entries, query)
  const shown = filterActivity(entries, filter, query)
  const filtered = filter !== null || query.trim() !== ""
  const clearFilter = () => {
    setFilter(null)
    setQuery("")
  }
  const focusList = () => list.current?.focus()

  const log: ReactNode =
    shown.length > 0 ? (
      <ActivityLog entries={shown} openedAt={openedAt} actions={actions} />
    ) : (
      <div className="flex items-center gap-3 px-4 py-4 text-sm text-muted-foreground" data-testid="activity-empty">
        {filtered ? (
          <>
            <span className="min-w-0 truncate">{query.trim() === "" ? "Nothing of this kind" : `No match for “${query.trim()}”`}</span>
            <Button variant="link" size="xs" className="h-auto px-0" onClick={clearFilter}>
              Clear Filter
            </Button>
          </>
        ) : (
          "Nothing yet: what Branch Deck does in the repository shows here"
        )}
      </div>
    )
  const listClassName = "min-h-0 flex-1 overflow-y-auto pb-2 outline-none"
  return (
    <>
      {/* in the window the search and the chips lie on the canvas, as the title bar */}
      <div className={cn("flex flex-wrap items-center gap-x-3 gap-y-2", layout === "window" ? "px-1" : "shrink-0 px-3 pb-2")} data-testid="activity-filter">
        <SearchField
          value={query}
          onValueChange={setQuery}
          label="Filter by branch or text"
          onLeave={focusList}
          onArrowDown={focusList}
          slashFocus={slashFocus}
          className={layout === "window" ? "w-60" : "w-full"}
        />
        {/* the chosen one again goes back to All */}
        <FilterChips label="Kind" value={filter ?? ALL} fallback={ALL} onValueChange={chip => setFilter(chip === ALL ? null : (chip as ActivityFilter))}>
          <FilterChip value={ALL} count={counts.all} data-chip="all">
            All
          </FilterChip>
          {ACTIVITY_FILTERS.filter(chip => counts[chip] > 0 || filter === chip).map(chip => (
            <FilterChip key={chip} value={chip} count={counts[chip]} data-chip={chip}>
              {ACTIVITY_FILTER_LABELS[chip]}
            </FilterChip>
          ))}
        </FilterChips>
      </div>
      {layout === "window" ? (
        <Card ref={list} tabIndex={-1} className={cn(listClassName, "gap-0 py-0")} data-testid="activity-list">
          {log}
        </Card>
      ) : (
        <div ref={list} tabIndex={-1} className={listClassName} data-testid="activity-list">
          {log}
        </div>
      )}
    </>
  )
}
