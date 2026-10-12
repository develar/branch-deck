import { FolderClosedIcon, FolderOpenIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { LinkedText } from "@/components/commit/LinkedText"
import { getSimpleBranchName } from "@/lib/branch"

export interface BranchNameCellProps {
  name: string
  summary?: string
  expanded: boolean
  canExpand?: boolean
  /** Archived branches: show only the last path segment */
  simplified?: boolean
  onToggleExpanded: () => void
}

export function BranchNameCell({ name, summary, expanded, canExpand = true, simplified = false, onToggleExpanded }: BranchNameCellProps) {
  const FolderIcon = expanded ? FolderOpenIcon : FolderClosedIcon
  return (
    <td className="max-w-xs px-6 py-4">
      <div className="flex min-w-0 items-center gap-2">
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
        <LinkedText text={simplified ? getSimpleBranchName(name) : name} className="shrink-0 text-sm font-medium" />
        {summary && <span className="truncate text-xs text-muted-foreground">{summary}</span>}
      </div>
    </td>
  )
}
