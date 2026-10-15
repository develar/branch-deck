import type { ReactNode } from "react"
import { FolderClosedIcon, FolderOpenIcon } from "lucide-react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
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
  /** A mark after the name, e.g. "New" on a branch archived in this session */
  mark?: ReactNode
  onToggleExpanded: () => void
}

export function BranchNameCell({ name, summary, expanded, canExpand = true, simplified = false, depth = 0, issueKey = null, fixupCount = 0, mark, onToggleExpanded }: BranchNameCellProps) {
  const FolderIcon = expanded ? FolderOpenIcon : FolderClosedIcon
  const content = (
    // the rule spans the row height, under the icon of the row above (`--bd-nesting-step` in index.css)
    <div className={cn("flex min-w-0 items-center gap-2", depth > 0 && "bd-nesting-leaf")} data-testid={depth > 0 ? "nesting-rule" : undefined}>
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
      {summary && <BranchSummary summary={summary} />}
    </div>
  )
  return (
    // a nested row starts at its first rule (2rem), a top-level row at its folder icon
    <td className={cn("w-full max-w-0 py-4", depth > 0 ? "pr-6 pl-8" : "px-6")}>
      <NestingRules levels={depth - 1}>{content}</NestingRules>
    </td>
  )
}

/**
 * The rules of `levels` tree levels around `children`, one `--bd-nesting-step` apart, each over the whole height of its
 * `py-4` cell. The branch row draws the levels above its own; its expanded commits draw all of its levels, so the tree
 * continues past them.
 */
export function NestingRules({ levels, children }: { levels: number, children: ReactNode }) {
  let nested = children
  for (let level = 0; level < levels; level++) {
    nested = <div className="bd-nesting-level" data-testid="nesting-rule">{nested}</div>
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

/** Latest commit subject of the branch, with its issue links; a conventional-commit prefix is dimmed as in the commit rows. */
export function BranchSummary({ summary }: { summary: string }) {
  const { prefix, text } = splitSubjectPrefix(summary)
  return (
    <span className="truncate text-xs text-muted-foreground">
      {prefix != null && <span className="font-mono text-muted-foreground/70">{`${prefix} `}</span>}
      <LinkedText text={text} />
    </span>
  )
}
