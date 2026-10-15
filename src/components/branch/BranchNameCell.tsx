import type { ReactNode } from "react"
import { FolderClosedIcon, FolderOpenIcon } from "lucide-react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LinkedText } from "@/components/commit/LinkedText"
import { formatFixupCount } from "@/lib/autosquash"
import { getBranchSummary, getSimpleBranchName } from "@/lib/branch"
import type { SubjectParts } from "@/lib/commit"

export interface BranchNameCellProps {
  name: string
  summary?: string
  /** The title of the branch's description, shown as written instead of the summary */
  descriptionTitle?: string | null
  expanded: boolean
  canExpand?: boolean
  /** Archived branches: show only the last path segment */
  simplified?: boolean
  /** Under the row of its issue: indented one step, with the guide of the family under the icon of the row above */
  familyIndent?: 0 | 1
  /** The lanes of a table with stacks (`LaneCell`), before the folder button; the cell is their positioning box */
  lanes?: ReactNode
  /** The issue of the branch, shown as a chip next to the name */
  issueKey?: string | null
  /** Fixup commits to squash into commits of the branch */
  fixupCount?: number
  /** A mark after the name, e.g. "New" on a branch archived in this session */
  mark?: ReactNode
  onToggleExpanded: () => void
}

export function BranchNameCell({ name, summary = "", descriptionTitle = null, expanded, canExpand = true, simplified = false, familyIndent = 0, lanes, issueKey = null, fixupCount = 0, mark, onToggleExpanded }: BranchNameCellProps) {
  const FolderIcon = expanded ? FolderOpenIcon : FolderClosedIcon
  const shownSummary = getBranchSummary(summary, descriptionTitle)
  const content = (
    // without lanes, the family guide spans the row height, under the icon of the row above (`bd-family-guide` in index.css)
    <div className={cn("flex min-w-0 items-center gap-2", lanes === undefined && familyIndent > 0 && "bd-family-guide")} data-testid={lanes === undefined && familyIndent > 0 ? "family-guide" : undefined}>
      {lanes}
      {canExpand && (
        <Tooltip>
          <TooltipTrigger
            render={(
              <Button
                variant="ghost"
                size="icon-xs"
                className="shrink-0 text-muted-foreground aria-expanded:text-foreground"
                // every row has one: the label names the branch
                aria-label={`${expanded ? "Collapse" : "Expand"} ${name}`}
                aria-expanded={expanded}
                onClick={(event) => {
                  event.stopPropagation()
                  onToggleExpanded()
                }}
              />
            )}
          >
            <FolderIcon />
          </TooltipTrigger>
          <TooltipContent>{expanded ? "Collapse" : "Expand"}</TooltipContent>
        </Tooltip>
      )}
      {/* an issue key in the name is a link, but the name keeps the foreground colour */}
      <LinkedText text={simplified ? getSimpleBranchName(name) : name} className="shrink-0 text-sm font-medium" linkClassName="text-foreground hover:underline" />
      {issueKey !== null && <BranchIssueBadge issueKey={issueKey} />}
      {fixupCount > 0 && <Badge variant="outline" className="shrink-0 text-muted-foreground" data-testid="branch-fixup-count">{formatFixupCount(fixupCount)}</Badge>}
      {mark}
      {/* the rest of the row; below 6rem the summary would be a letter or two, so it drops out */}
      {shownSummary !== null && (
        <span className="@container min-w-0 flex-1" data-testid="branch-summary-slot">
          <BranchSummary summary={shownSummary} className="hidden @min-[6rem]:block" />
        </span>
      )}
    </div>
  )
  if (lanes !== undefined) {
    // the lane column starts at --bd-lane-start; its lines span the cell
    return <td className="relative w-full max-w-0 py-(--bd-cell-y) pr-6 pl-(--bd-lane-start)">{content}</td>
  }
  return (
    // a family member starts at its guide (2rem), any other row at its folder icon
    <td className={cn("w-full max-w-0 py-(--bd-cell-y)", familyIndent > 0 ? "pr-6 pl-8" : "px-6")}>{content}</td>
  )
}

/** The issue of a branch as a chip next to its name. */
export function BranchIssueBadge({ issueKey }: { issueKey: string }) {
  return (
    <Badge variant="outline" className="shrink-0" data-testid="branch-issue-key">
      <LinkedText text={issueKey} linkClassName="text-muted-foreground hover:underline" />
    </Badge>
  )
}

/** The summary of a branch (`getBranchSummary`) with its issue links; a prefix is dimmed as in the commit rows. */
export function BranchSummary({ summary: { prefix, text }, className }: { summary: SubjectParts, className?: string }) {
  return (
    <span className={cn("truncate text-xs text-muted-foreground", className)} data-testid="branch-summary">
      {prefix != null && <span className="font-mono text-muted-foreground/70">{`${prefix} `}</span>}
      <LinkedText text={text} />
    </span>
  )
}
