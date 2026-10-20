import { createContext, use, useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import type { BranchTrack, BranchTrackGroup } from "@/lib/branch-columns"
import { ColumnRegistry, TRACK_CONTENT_CLASS, TRACK_ITEM_CLASS } from "@/lib/branch-columns"

const ColumnRegistryContext = createContext<ColumnRegistry | null>(null)

/**
 * The branch view: the table and its stack islands, one card after the other, whose columns line up
 * (`ColumnRegistry`): the widths of the tracks are CSS variables of this element
 */
export function BranchColumnsRoot({ children }: { children: ReactNode }) {
  const [registry] = useState(() => new ColumnRegistry())
  return (
    <div
      ref={element => {
        registry.setRoot(element)
        return () => registry.setRoot(null)
      }}
      className="flex flex-col gap-2"
      data-testid="branch-view"
    >
      <ColumnRegistryContext value={registry}>{children}</ColumnRegistryContext>
    </div>
  )
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
