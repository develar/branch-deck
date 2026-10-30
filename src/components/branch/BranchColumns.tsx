import { createContext, use, useEffect, useLayoutEffect, useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { saveSettledLayout, settledTracks, useLayoutSettled, useSettledLayout } from "@/hooks/useSettledLayout"
import type { BranchTrack, BranchTrackGroup } from "@/lib/branch-columns"
import { ColumnRegistry, TRACK_CONTENT_CLASS, TRACK_ITEM_CLASS } from "@/lib/branch-columns"
import { useBranchSync } from "@/stores/branch-sync"
import { useSelectedProjectPath } from "@/stores/repository"

const ColumnRegistryContext = createContext<ColumnRegistry | null>(null)

/**
 * The branch view: the table and its stack islands, one card after the other, whose columns line up
 * (`ColumnRegistry`): the widths of the tracks are CSS variables of this element. Until the facts of the remote are in,
 * the tracks are at least as wide as once they were in the last time (`useSettledLayout`); then the view keeps its
 * widths for the next start. A running sync moves no column: while it runs a track only widens. `data-wide` while the
 * view has the room for the words of a row (`WIDE_REM`). The view contains its layout and its inline size: a text of a
 * row that changes its width lays out the view, not the boxes of the window around it (the panels of the main window,
 * the scroller), as the width of the view comes from the window and not from its rows
 * (docs/design/ui-theme.md#branch-columns, "Contained")
 */
export function BranchColumnsRoot({ children }: { children: ReactNode }) {
  const [registry] = useState(() => new ColumnRegistry())
  useSettledColumns(registry)
  return (
    <div
      ref={element => {
        registry.setRoot(element)
        return () => registry.setRoot(null)
      }}
      // the group of the words a row shows where the view has the room ("with children" of a push): `data-wide`, which
      // the registry sets from the width of the view; contained, but no query container (one restyles every row at
      // each width)
      className="group/branch-view flex flex-col gap-2 contain-inline-size contain-layout"
      data-testid="branch-view"
    >
      <ColumnRegistryContext value={registry}>{children}</ColumnRegistryContext>
    </div>
  )
}

/** The floors of the tracks while the facts of the remote come, the hold while a sync runs, and what they settle at */
function useSettledColumns(registry: ColumnRegistry): void {
  const path = useSelectedProjectPath()
  const settled = useLayoutSettled()
  const syncing = useBranchSync(state => state.isSyncing)
  const floors = useSettledLayout()?.tracks ?? null
  // before the first paint: the first frame stands where the settled one will, the end of a run where the run did
  useLayoutEffect(() => {
    registry.setLayout(settled ? null : floors, syncing)
  }, [registry, settled, floors, syncing])
  useEffect(() => {
    if (!settled || path === undefined) {
      return
    }
    // the widths the settled content gives, once its frame is laid out
    const frame = requestAnimationFrame(() => saveSettledLayout(path, { tracks: settledTracks(registry.measured()) }))
    return () => cancelAnimationFrame(frame)
  }, [registry, settled, path])
}

/**
 * An item of `track`, as wide as the widest item of the track in the view; its content measured at its natural width,
 * capped where the track truncates. Nothing inside, it is 0 wide, and so is the track when no row fills it.
 */
export function ColumnTrack({ track, className, children }: { track: BranchTrack; className?: string; children: ReactNode }) {
  const registry = use(ColumnRegistryContext)
  return (
    <div className={cn("flex shrink-0", TRACK_ITEM_CLASS[track])} data-track={track}>
      <div
        ref={element => (element === null || registry === null ? undefined : registry.observe(element, track))}
        className={cn("flex w-max min-w-0 items-center", TRACK_CONTENT_CLASS[track], className)}
      >
        {children}
      </div>
    </div>
  )
}

/** The label of a column heading `group`: the items of the group in every row are at least as wide */
export function ColumnLabel({ group, children }: { group: BranchTrackGroup; children: ReactNode }) {
  const registry = use(ColumnRegistryContext)
  return (
    <span ref={element => (element === null || registry === null ? undefined : registry.observe(element, `group-${group}`))} className="inline-block">
      {children}
    </span>
  )
}
