import type { ReactNode } from "react"
import { FolderClosedIcon, FolderOpenIcon } from "lucide-react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { LinkedText } from "@/components/commit/LinkedText"
import { formatFixupCount } from "@/lib/autosquash"
import { getSimpleBranchName } from "@/lib/branch"
import { splitSubjectPrefix } from "@/lib/commit"

export interface BranchNameCellProps {
  name: string
  summary?: string
  expanded: boolean
  canExpand?: boolean
  /** Archived branches: show only the last path segment */
  simplified?: boolean
  /**
   * Levels the row is nested: under the row of its issue and under the branches it stacks on. Each level indents it,
   * with a rule under the icon of the row above
   */
  depth?: number
  /** The issue of the branch, shown as a chip next to the name */
  issueKey?: string | null
  /** Fixup commits to squash into commits of the branch */
  fixupCount?: number
  onToggleExpanded: () => void
}

export function BranchNameCell({ name, summary, expanded, canExpand = true, simplified = false, depth = 0, issueKey = null, fixupCount = 0, onToggleExpanded }: BranchNameCellProps) {
  const FolderIcon = expanded ? FolderOpenIcon : FolderClosedIcon
  const content = (
    // the rule spans the row height, below the icon of the row above
    <div className={cn("flex min-w-0 items-center gap-2", depth > 0 && "-my-4 border-l border-border py-4 pl-4", depth > 1 && "ml-3")}>
      {canExpand && (
        <Button
          variant="ghost"
          size="icon-xs"
          className="shrink-0 text-primary hover:text-primary aria-expanded:text-primary"
          aria-label={expanded ? "Collapse" : "Expand"}
          aria-expanded={expanded}
          onClick={(event) => {
            event.stopPropagation()
            onToggleExpanded()
          }}
        >
          <FolderIcon />
        </Button>
      )}
      {/* an issue key in the name is a link, but the name keeps the foreground colour */}
      <LinkedText text={simplified ? getSimpleBranchName(name) : name} className="shrink-0 text-sm font-medium" linkClassName="text-foreground hover:underline" />
      {issueKey !== null && <BranchIssueBadge issueKey={issueKey} />}
      {fixupCount > 0 && <Badge variant="outline" className="shrink-0 text-muted-foreground" data-testid="branch-fixup-count">{formatFixupCount(fixupCount)}</Badge>}
      {summary && <BranchSummary summary={summary} />}
    </div>
  )
  return (
    <td className={cn("w-full max-w-0 py-4", depth > 0 ? "pr-6 pl-9" : "px-6")}>
      <NestingRules depth={depth}>{content}</NestingRules>
    </td>
  )
}

/** The rules of the levels above the row's own: one per level, each under the icon of the row of that level */
function NestingRules({ depth, children }: { depth: number, children: ReactNode }) {
  let nested = children
  for (let level = depth - 1; level >= 1; level--) {
    nested = <div className={cn("-my-4 border-l border-border py-4 pl-4", level > 1 && "ml-3")}>{nested}</div>
  }
  return nested
}

/** The issue of a branch as a chip next to its name. */
export function BranchIssueBadge({ issueKey }: { issueKey: string }) {
  return (
    <Badge variant="outline" className="shrink-0" data-testid="branch-issue-key">
      <LinkedText text={issueKey} linkClassName="text-muted-foreground hover:underline" />
    </Badge>
  )
}

/** Latest commit subject of the branch; a conventional-commit prefix is dimmed as in the commit rows. */
export function BranchSummary({ summary }: { summary: string }) {
  const { prefix, text } = splitSubjectPrefix(summary)
  return (
    <span className="truncate text-xs text-muted-foreground">
      {prefix != null && <span className="font-mono text-muted-foreground/70">{`${prefix} `}</span>}
      {text}
    </span>
  )
}
