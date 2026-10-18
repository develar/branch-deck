import type { ReactNode } from "react"
import { cn } from "cn"

export interface SectionLabelProps {
  title: string
  /** "Files 4": how many the section lists */
  count?: number
  /** "Files 2 of 18": the count of all, while a search shows a part */
  of?: number
  /** On the right: what the section sums up (the line counts of its files), muted */
  children?: ReactNode
  id?: string
  className?: string
}

/**
 * "Files 4 … +12 −3": the head of a section inside an island (the files and commits of a review, the files of a commit
 * preview, a branch of a stack in the commit column), muted `text-xs font-medium` as `PopoverSectionTitle`; not a
 * heading, the island has its title
 */
export function SectionLabel({ title, count, of, children, id, className }: SectionLabelProps) {
  return (
    <div id={id} className={cn("flex min-w-0 items-center gap-1.5 text-xs font-medium text-muted-foreground", className)}>
      <span className="truncate">{title}</span>
      {count !== undefined && <span className="tabular-nums">{of === undefined ? count : `${count} of ${of}`}</span>}
      {children !== undefined && <span className="ml-auto font-normal">{children}</span>}
    </div>
  )
}
