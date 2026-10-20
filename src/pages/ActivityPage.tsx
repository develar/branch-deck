import { useEffect, useRef, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ActivityLog } from "@/components/repository/ActivityLog"
import type { ActivityLogActions } from "@/components/repository/ActivityLog"
import { ChoiceChip } from "@/components/shared/ChoiceChip"
import { KeyHint, PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { SearchField } from "@/components/shared/SearchField"
import { WindowPage } from "@/components/shared/WindowPage"
import { askMainWindow, useLinkedFeedback, useLinkedState } from "@/hooks/useMainWindowLink"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { ACTIVITY_FILTER_LABELS, ACTIVITY_FILTERS, ACTIVITY_WINDOW, activityWindowTitle, countActivityFilters, filterActivity } from "@/lib/activity-window"
import type { ActivityFilter, ActivityWindowData, ActivityWindowFeedback, ActivityWindowRequest } from "@/lib/activity-window"
import { logError, notifyError } from "@/lib/errors"

/**
 * The Activity window (spec/repository/activity.spec.md): the log of the repository the main window shows, in one
 * island under a search and the chips of the kinds on the canvas, Clear Log in the footer. The main window owns the log
 * and sends it over the link of the window; a branch name, Undo, Dismiss and Clear Log run there. Each open starts over
 * with the chip it names and an empty search.
 */
export function ActivityPage() {
  const data = useSubWindowData<ActivityWindowData>()
  if (data === null) {
    return null
  }
  return <ActivityWindow key={data.openedAt} data={data} />
}

function ActivityWindow({ data }: { data: ActivityWindowData }) {
  // the window follows the repository the main window shows
  const state = useLinkedState(ACTIVITY_WINDOW, data.state, { anyRepository: true })
  const [filter, setFilter] = useState<ActivityFilter | null>(data.kind)
  const [query, setQuery] = useState("")
  const listRef = useRef<HTMLDivElement>(null)
  const { repositoryPath } = state
  useEffect(() => {
    getCurrentWindow()
      .setTitle(activityWindowTitle(repositoryPath))
      .catch((error: unknown) => logError("Cannot set the title of Activity", error))
  }, [repositoryPath])
  useLinkedFeedback<ActivityWindowFeedback>(ACTIVITY_WINDOW, feedback => notifyError(feedback.title, feedback.message))

  const ask = (request: { reveal: string } | { undo: string } | { dismiss: true } | { clear: true }) =>
    void askMainWindow<ActivityWindowRequest>(ACTIVITY_WINDOW, { repositoryPath, ...request })
  const actions: ActivityLogActions = {
    branches: new Set(state.branches),
    undismissed: new Set(state.undismissed),
    links: state.links,
    onReveal: name => ask({ reveal: name }),
    onUndo: entry => ask({ undo: entry.id }),
    onDismiss: () => ask({ dismiss: true }),
  }
  const counts = countActivityFilters(state.entries, query)
  const entries = filterActivity(state.entries, filter, query)
  const filtered = filter !== null || query.trim() !== ""
  const clearFilter = () => {
    setFilter(null)
    setQuery("")
  }
  const focusList = () => listRef.current?.focus()

  return (
    <WindowPage
      title="Activity"
      body="fill"
      data-testid="activity-window"
      footer={
        <PreviewDialogFooter
          className="border-t-0 px-1 py-0"
          start={
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button size="xs" variant="ghost" disabled={state.entries.length === 0} focusableWhenDisabled data-testid="activity-clear" onClick={() => ask({ clear: true })} />
                }
              >
                Clear Log
              </TooltipTrigger>
              <TooltipContent>Forget the whole log of the repository, whatever the filter shows</TooltipContent>
            </Tooltip>
          }
        >
          <KeyHint keys={<Kbd>Esc</Kbd>}>Close</KeyHint>
        </PreviewDialogFooter>
      }
    >
      {/* the search and the chips lie on the canvas, as the title bar */}
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 px-1" data-testid="activity-filter">
        <SearchField value={query} onValueChange={setQuery} label="Filter by branch or text" onLeave={focusList} onArrowDown={focusList} className="w-60" />
        <div role="group" aria-label="Kind" className="flex flex-wrap items-center gap-1">
          <ChoiceChip chosen={filter === null} data-chip="all" onClick={() => setFilter(null)}>
            All
            <span className="tabular-nums opacity-70">{counts.all}</span>
          </ChoiceChip>
          {ACTIVITY_FILTERS.filter(chip => counts[chip] > 0 || filter === chip).map(chip => (
            // the chosen one again goes back to All
            <ChoiceChip key={chip} chosen={filter === chip} data-chip={chip} onClick={() => setFilter(filter === chip ? null : chip)}>
              {ACTIVITY_FILTER_LABELS[chip]}
              <span className="tabular-nums opacity-70">{counts[chip]}</span>
            </ChoiceChip>
          ))}
        </div>
      </div>
      <Card ref={listRef} tabIndex={-1} className="min-h-0 flex-1 gap-0 overflow-y-auto py-0 pb-2 outline-none" data-testid="activity-list">
        {entries.length > 0 ? (
          <ActivityLog entries={entries} openedAt={data.openedAt} actions={actions} />
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
        )}
      </Card>
    </WindowPage>
  )
}
